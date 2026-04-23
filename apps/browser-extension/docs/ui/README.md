# Browser Extension UI

The popup interface is a Svelte 5 app at
`src/entrypoints/popup/App.svelte`. It covers:

- Session creation / invitation flow (Ext-3)
- DKG progress + group-public-key display
- Signing confirm / progress / complete modals (Ext-2)
- Chain + network selection
- WebRTC / WebSocket status indicators

Design intent: single-panel popup that surfaces session state
and one actionable affordance at a time. State transitions are
driven from the background service worker via
`chrome.runtime.connect({name: "popup"})` messages.