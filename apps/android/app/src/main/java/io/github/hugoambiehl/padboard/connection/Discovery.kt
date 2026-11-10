package io.github.hugoambiehl.padboard.connection

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import androidx.annotation.RequiresApi
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.util.concurrent.Executors
import java.util.logging.Logger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** A PC announcing Padboard on the local network (mDNS TXT keys in `crates/protocol`). */
data class DiscoveredPc(
    val serverId: String,
    val name: String,
    val protocolVersion: Int?,
    /** Addresses, IPv4 first. */
    val hosts: List<String>,
    val port: Int
) {
    val endpoints: List<Endpoint> get() = hosts.map { Endpoint(it, port) }
}

/** The PCs currently visible on the network. */
interface PcDirectory {
    val pcs: StateFlow<List<DiscoveredPc>>
}

/**
 * mDNS discovery with Android's [NsdManager] (no Play Services).
 *
 * Before Android 14 a resolution yields a single address, whichever comes
 * first: addresses seen for a service are accumulated, IPv4 first. Link-local
 * IPv6 addresses are dropped (they need a scope that URLs cannot carry).
 */
class NsdDirectory(context: Context) : PcDirectory {
    private val nsd = context.applicationContext.getSystemService(NsdManager::class.java)
    private val _pcs = MutableStateFlow<List<DiscoveredPc>>(emptyList())
    override val pcs: StateFlow<List<DiscoveredPc>> = _pcs.asStateFlow()

    /** Service name → PC, from the resolutions. */
    private val byService = mutableMapOf<String, DiscoveredPc>()

    /** Before API 34 only one resolution may run at a time. */
    private val pending = ArrayDeque<NsdServiceInfo>()
    private var resolving = false
    private var listener: NsdManager.DiscoveryListener? = null
    private val callbackExecutor = Executors.newSingleThreadExecutor()
    private val serviceCallbacks = mutableMapOf<String, NsdManager.ServiceInfoCallback>()

    @Synchronized
    fun start() {
        if (listener != null) return
        val discovery =
            object : NsdManager.DiscoveryListener {
                override fun onDiscoveryStarted(serviceType: String) = Unit

                override fun onDiscoveryStopped(serviceType: String) = Unit

                override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                    log.warning("mDNS discovery failed to start: $errorCode")
                    synchronized(this@NsdDirectory) { listener = null }
                }

                override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit

                override fun onServiceFound(service: NsdServiceInfo) = resolve(service)

                override fun onServiceLost(service: NsdServiceInfo) = forget(service.serviceName)
            }
        listener = discovery
        nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discovery)
    }

    @Synchronized
    fun stop() {
        listener?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        listener = null
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            serviceCallbacks.values.forEach { runCatching { nsd.unregisterServiceInfoCallback(it) } }
        }
        serviceCallbacks.clear()
        pending.clear()
        byService.clear()
        _pcs.value = emptyList()
    }

    @Synchronized
    private fun resolve(service: NsdServiceInfo) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            follow(service)
            return
        }
        pending.addLast(service)
        resolveNext()
    }

    /** API 34+: follows the service, with all its addresses. */
    @RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
    private fun follow(service: NsdServiceInfo) {
        if (service.serviceName in serviceCallbacks) return
        val callback =
            object : NsdManager.ServiceInfoCallback {
                override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {
                    log.warning("mDNS resolution failed: $errorCode")
                }

                override fun onServiceUpdated(info: NsdServiceInfo) =
                    record(service.serviceName, info, info.hostAddresses)

                override fun onServiceLost() = forget(service.serviceName)

                override fun onServiceInfoCallbackUnregistered() = Unit
            }
        serviceCallbacks[service.serviceName] = callback
        nsd.registerServiceInfoCallback(service, callbackExecutor, callback)
    }

    @Synchronized
    private fun resolveNext() {
        if (resolving) return
        val service = pending.removeFirstOrNull() ?: return
        resolving = true
        @Suppress("DEPRECATION") // replaced by registerServiceInfoCallback on API 34+ (see follow)
        nsd.resolveService(
            service,
            object : NsdManager.ResolveListener {
                override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) = done()

                override fun onServiceResolved(info: NsdServiceInfo) {
                    @Suppress("DEPRECATION")
                    record(info.serviceName, info, listOfNotNull(info.host))
                    done()
                }

                private fun done() {
                    synchronized(this@NsdDirectory) { resolving = false }
                    resolveNext()
                }
            }
        )
    }

    @Synchronized
    private fun record(serviceName: String, info: NsdServiceInfo, addresses: List<InetAddress>) {
        val attributes = info.attributes
        fun text(key: String) = attributes[key]?.toString(Charsets.UTF_8)
        val serverId = text(TXT_SERVER_ID) ?: return
        val usable = addresses.filter { it is Inet4Address || (it is Inet6Address && !it.isLinkLocalAddress) }
        val previous = byService[serviceName]?.hosts.orEmpty()
        val hosts =
            (previous + usable.mapNotNull { it.hostAddress?.substringBefore('%') })
                .distinct()
                .sortedBy { ':' in it } // IPv4 first, stable otherwise
        if (hosts.isEmpty()) return
        byService[serviceName] =
            DiscoveredPc(
                serverId = serverId,
                name = text(TXT_SERVER_NAME) ?: serviceName,
                protocolVersion = text(TXT_PROTOCOL_VERSION)?.toIntOrNull(),
                hosts = hosts,
                port = info.port
            )
        publish()
    }

    @Synchronized
    private fun forget(serviceName: String) {
        if (byService.remove(serviceName) != null) publish()
    }

    /** One entry per PC: a PC seen on several interfaces merges its addresses. */
    private fun publish() {
        val merged =
            byService.values
                .groupBy { it.serverId }
                .map { (_, entries) -> entries.first().copy(hosts = entries.flatMap { it.hosts }.distinct()) }
                .sortedBy { it.name.lowercase() }
        _pcs.update { merged }
    }

    private companion object {
        const val SERVICE_TYPE = "_padboard._tcp"
        const val TXT_PROTOCOL_VERSION = "v"
        const val TXT_SERVER_ID = "id"
        const val TXT_SERVER_NAME = "name"
        val log: Logger = Logger.getLogger(NsdDirectory::class.java.name)
    }
}
