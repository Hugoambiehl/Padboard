import { describe, expect, it } from "vitest";
import { appSuggestion, APP_ICON } from "./appSuggestion";

describe("appSuggestion", () => {
  it("names a Windows program after its file", () => {
    expect(appSuggestion("C:\\Program Files\\obs-studio\\bin\\64bit\\obs64.exe")).toEqual({
      label: "Obs64",
      icon: "Video",
    });
    expect(
      appSuggestion("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Discord.lnk"),
    ).toEqual({ label: "Discord", icon: "MessageCircle" });
  });

  it("reads Linux .desktop ids and programs", () => {
    expect(appSuggestion("/usr/share/applications/org.gnome.Nautilus.desktop")).toEqual({
      label: "Nautilus",
      icon: "Folder",
    });
    expect(appSuggestion("/var/lib/snapd/desktop/applications/firefox_firefox.desktop")).toEqual({
      label: "Firefox",
      icon: "Globe",
    });
    expect(appSuggestion("/usr/bin/gnome-calculator")).toEqual({
      label: "Gnome calculator",
      icon: "Calculator",
    });
  });

  it("falls back to the application icon", () => {
    expect(appSuggestion("/opt/tool/MyTool.AppImage")).toEqual({ label: "MyTool", icon: APP_ICON });
  });
});
