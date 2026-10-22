const RED = "oklch(55% 0.19 28)";
const GREEN = "oklch(46% 0.1 160)";
const RED_SOFT = "oklch(55% 0.19 28 / 0.35)";
const GREEN_SOFT = "oklch(46% 0.1 160 / 0.35)";
const RED_HEX = "#b23a2b";
const GREEN_HEX = "#2f6f5e";
const RED_HEX_SOFT = "rgba(178,58,43,0.35)";
const GREEN_HEX_SOFT = "rgba(47,111,94,0.35)";

export function applyColorPreference(upColor: string) {
  const greenUp = upColor === "green";
  const root = document.documentElement;
  root.dataset.upColor = upColor;
  const dark = root.dataset.theme === "dark";
  const red = dark ? "#ef8276" : RED;
  const green = dark ? "#7bbba3" : GREEN;
  const redHex = dark ? "#ef8276" : RED_HEX;
  const greenHex = dark ? "#7bbba3" : GREEN_HEX;
  root.style.setProperty("--color-up", greenUp ? green : red);
  root.style.setProperty("--color-down", greenUp ? red : green);
  root.style.setProperty("--color-up-soft", greenUp ? GREEN_SOFT : RED_SOFT);
  root.style.setProperty("--color-down-soft", greenUp ? RED_SOFT : GREEN_SOFT);
  root.style.setProperty("--chart-up", greenUp ? greenHex : redHex);
  root.style.setProperty("--chart-down", greenUp ? redHex : greenHex);
  root.style.setProperty("--chart-up-soft", greenUp ? GREEN_HEX_SOFT : RED_HEX_SOFT);
  root.style.setProperty("--chart-down-soft", greenUp ? RED_HEX_SOFT : GREEN_HEX_SOFT);
  window.dispatchEvent(new Event("alphak-theme-change"));
}

export function applyDensity(density: string) {
  document.documentElement.dataset.density = density === "compact" ? "compact" : "comfortable";
}

let themeMedia: MediaQueryList | undefined;
let themeListener: (() => void) | undefined;

export function applyTheme(theme: string) {
  if (themeMedia && themeListener) themeMedia.removeEventListener("change", themeListener);
  themeMedia = window.matchMedia("(prefers-color-scheme: dark)");
  const update = () => {
    const dark = theme === "dark" || (theme === "system" && themeMedia!.matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
    applyColorPreference(document.documentElement.dataset.upColor ?? "red");
  };
  themeListener = update;
  if (theme === "system") themeMedia.addEventListener("change", update);
  update();
}
