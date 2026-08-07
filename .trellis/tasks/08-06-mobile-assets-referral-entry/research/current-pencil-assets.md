# Current Pencil Assets Mapping

## Sources

- Guest light: `CUK3y`
- Guest dark: `i6YDBr`
- Member light empty state: `p61z2Q`
- Member dark populated state: `Q4JYj`
- Construction scripts: `mobile/pencil/scripts/17-assets-member-immersive.js` through `25-assets-guest-immersive.js`

## Geometry and behavior

- Root title is `22px / 750`, with the member-only balance visibility action.
- The immersive card sits in a transparent 16px outer gutter and is 236px high with the project large radius.
- Guest card contains a kicker, login title, supporting copy, and a 50px glass login action only.
- Member card contains total-value and today-return columns plus four 66px action cells.
- Member holdings are real rows sorted by estimated value; the light reference demonstrates the zero-holding state, while the dark reference demonstrates populated rows.
- Funding tools remain three 52px rows below holdings.
- The shared five-entry dock is shell-owned and must not be reimplemented in the view.

## Production mapping constraints

- Use no demo balances or demo return values from the Pencil file.
- Copy the light/dark card source images into tracked production assets.
- Keep guest content minimal; it should not reveal protected action surfaces before login.
- Retain existing wallet/market/transfer behavior and the accessible confirmation sheet.
- The existing `referrals` route and page are complete; add only a profile entry using the existing localized label/description.
