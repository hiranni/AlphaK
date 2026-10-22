# AlphaK market terminal

Design intent: a compact A-share research terminal. Put prices, charts and tables first.

- App structure: Workbench; existing navigation and routes.
- Palette: cool navy surfaces, blue controls; red/green reserved for market movement.
- Typography: IBM Plex Sans / system Chinese sans; IBM Plex Mono for numeric data.
- Page titles: one short label, 18–20 px. No slogans, hero sections or introductory essays.
- Panels: 14 px titles, thin borders, 12–14 px padding; no decoration or reveal animation.
- Theme: dark default for new installations, cool light variant; respect saved preferences.
- Truthfulness: market overview excludes demo prices and demo sectors. Missing values remain unavailable. Sample breadth is explicitly scoped to local real quotes, never presented as whole-market breadth.
- Responsive: panels stack on narrow screens; tables scroll within their own containers.
- Tokens: apps/web/src/tokens.css. Root tokens.css mirrors the same system.

Design review: philosophy 5, hierarchy 5, execution 4, specificity 5, restraint 5, variety 3.
