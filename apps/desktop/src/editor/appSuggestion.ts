// "Lancer une application": a label and an icon proposed from the program
// the user picked (an .exe or .lnk on Windows, a .desktop entry or a
// program on Linux).

/** Lucide icons for well-known programs, by a word of their file name. */
const ICONS: [RegExp, string][] = [
  [/firefox|chrome|chromium|edge|brave|opera|vivaldi|browser/, "Globe"],
  [/obs/, "Video"],
  [/discord|slack|teams|telegram|signal|whatsapp|element/, "MessageCircle"],
  [/steam|lutris|heroic|epic|game/, "Gamepad2"],
  [/code|codium|studio|idea|pycharm|zed|sublime|notepad\+\+/, "Code"],
  [/terminal|konsole|ptyxis|alacritty|kitty|wezterm|powershell|cmd/, "Terminal"],
  [/spotify|music|rhythmbox|vlc|audacious|deezer/, "Music"],
  [/explorer|nautilus|files|dolphin|thunar|nemo/, "Folder"],
  [/thunderbird|outlook|mail|evolution|geary/, "Mail"],
  [/calc/, "Calculator"],
  [/gimp|krita|inkscape|photo|paint/, "Image"],
  [/word|writer|libreoffice|notepad|gedit|text/, "FileText"],
];

/** The default icon of an application button. */
export const APP_ICON = "AppWindow";

/** Label and icon proposed for the program at `path`. */
export function appSuggestion(path: string): { label: string; icon: string } {
  const file = path.split(/[\\/]/).pop() ?? path;
  let name = file.replace(/\.(exe|lnk|desktop|appimage|sh)$/i, "");
  if (/\.desktop$/i.test(file)) {
    // Reverse-DNS ids (org.gnome.Nautilus → Nautilus), snaps
    // (firefox_firefox → firefox).
    name = name.split(".").pop() ?? name;
    name = name.split("_").pop() ?? name;
  }
  name = name.replace(/[-_]+/g, " ").trim() || file;
  const label = name.charAt(0).toUpperCase() + name.slice(1);
  const lower = name.toLowerCase();
  const icon = ICONS.find(([words]) => words.test(lower))?.[1] ?? APP_ICON;
  return { label, icon };
}
