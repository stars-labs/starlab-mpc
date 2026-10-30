# Primary competitor visual reference supplement

Read-only designer audit, 4 October 2026. No repository or production output changes. Starlab accepted evidence remains 198 extension captures and 58 desktop captures. This supplement resolves the previous inability of the web text reader to inspect Rabby's rendered site.

## Evidence actually inspected

Actual Chromium browser session `rabby-audit` opened https://rabby.io/. Full rendered page screenshot: `/tmp/starlab-competitor-audit/rabby-official-site.png`; rendered text: `rabby-site-text.txt`. The page contains marketing carousel thumbnails, not an authenticated live wallet. The following exact image URLs were extracted from that rendered page and downloaded unchanged:

- Send: https://static-assets.rabby.io/files/0feab942-cccf-4034-a652-3d478339b0bb.png → `rabby-send-official-thumbnail.png`.
- Connect: https://static-assets.rabby.io/files/47cf47d9-ca2e-46d5-862b-cf3602f2492d.png → `rabby-connect-official-thumbnail.png`.
- Approvals: https://static-assets.rabby.io/files/32bb2ac2-060d-4cb8-8faf-24c2cd323b6b.png → `rabby-approvals-official-thumbnail.png`.
- Portfolio: https://static-assets.rabby.io/files/80a81992-2394-4405-81be-afce56c4ba93.png → `rabby-portfolio-official-thumbnail.png`.
- Swap: https://static-assets.rabby.io/files/1be93705-8648-465c-b02e-9416d7c39820.png → `rabby-swap-official-thumbnail.png`. This is explicitly swap, not approvals; used only to inspect token/chain identity styling, not to expand Starlab features.

Rabby bundled home preview from its official repository: https://github.com/RabbyHub/Rabby/blob/develop/src/ui/assets/new-user-import/home-preview.png ; actual image https://raw.githubusercontent.com/RabbyHub/Rabby/develop/src/ui/assets/new-user-import/home-preview.png → `rabby-home-official-repo.png`.

Phantom primary guide https://phantom.com/learn/guides/how-to-send-tokens-to-an-exchange contains the inspected image https://sanity-proxy-v2.phantom.app/images/3nm6d03a/production/8cc49f6901a958ec6a37539899fd6a53e6e6a639-1920x1080.png → `phantom-send-official-guide.png`. The guide is an older UI illustration; its text naming only three supported chains must not be treated as current product coverage.

MetaMask primary guide rechecked: https://support.metamask.io/manage-crypto/move-crypto/send/how-to-send-tokens-from-your-metamask-wallet . Its current text supports network/token selection, recipient/amount entry, fee review, confirmation and activity status. This audit did not obtain a MetaMask screenshot; do not claim a visual screenshot inspection for MetaMask.

## Supported comparisons against Starlab's accepted matrix

| Matrix functions | Actual competitor observation | Starlab assessment |
|---|---|---|
| Home, wallet/account selection, primary actions | Rabby official repo preview shows Ledger1 name, short address, separate copy, chain marks and labeled action grid. Send/Receive have recognizable icons and labels. | Accepted Starlab wallet/account identity and chain pill precede funds; Send/Receive are prominent. Secondary tools are grouped. Desktop dedicated navigation keeps forms out of the home. No blocking hierarchy mismatch found. |
| Portfolio, token identity, Send | Rabby Send thumbnail shows USDC mark with dominant -4,356.12 USDC amount, Ethereum mark/name, recipient and a $1.12 fee row. Phantom guide shows non-native USDT marks with chain corner badges, native ETH without a chain badge, amount/Max and fiat secondary. | Starlab native/canonical contract marks, chain badges, neutral unknown-token fallback, amount hierarchy and fee review follow these supported dimensions. Starlab uses truthful explicit preview-unavailable text instead of claiming competitor transaction simulation. |
| Send validation/error, approval actions | Rabby Send image visibly states Gas balance is not enough; Sign is pale/disabled and Cancel remains separate. | Starlab reviewed input/error and fee review fixtures preserve explanatory inline state and separated cancel/primary action. State is not conveyed by color alone. Actual submitted/receipt correctness remains a separate real-chain gate. |
| dApp connection | Rabby Connect image puts https://app.fakedapp.xyz prominently, shows Listed by and Flagged by Rabby / Yes risk information, then Connect/Cancel. It does NOT show account/network choices in this thumbnail. | Starlab origin and explicit account/network permissions are prominent; supported risk/result states are distinguished. No Rabby risk database exists in Starlab, so no equivalent reputation badge is claimed or required. |
| Token approvals | Rabby Approvals image shows DApp logo with small chain badge, named rows, counts, selection checkboxes and disabled Revoke footer. No spender detail/unlimited state is visible. | Starlab token/spender/chain identity, allowance amount, Unlimited label and Revoke busy/error are accepted. Different grouping is legitimate: existing Starlab backend exposes per-token allowance rows, not Rabby batch DApp grouping. No invented bulk-revoke requirement. |
| Empty/loading, Activity | Rabby marketing images contain gray placeholder blocks; GasAccount thumbnail separately states No history in the past 30 days with an illustration. | Placeholder marketing art is not proof of Rabby's runtime skeleton behavior. Starlab actual component fixture skeleton/error/empty states were inspected independently. No live competitor activity state claims. |
| DKG, threshold co-signing, save device share, refresh/recovery | These primary references do not demonstrate equivalent threshold lifecycle surfaces. | Starlab-specific states are assessed against honest protocol phase, threshold/device identity, save feedback and Ethereum retirement limitations, using the shared visual grammar. They are not claimed as competitor feature parity. |

## Review outcome

No new blocking visual issue found from these primary references. Existing accepted surfaces are aligned on identity hierarchy, token/chain artwork, review details, paired approval actions and clear state feedback. The official rendered site, its exact carousel image assets and official repository home preview were inspected. They remain marketing/bundled previews and do not establish every competitor runtime state. This supplements the earlier text-reader-based reference review; no presentation code changes were required.
