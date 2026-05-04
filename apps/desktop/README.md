# Zaned desktop

egui-based desktop client for Zaned.

## Authentication setup (one-time)

The desktop authenticates against the same Clerk instance as `zaned-server`
using OAuth 2.0 Authorization Code + PKCE.

### Clerk dashboard

1. Sign in to https://dashboard.clerk.com.
2. **OAuth Applications** → **Add OAuth Application**.
3. Set:
   - **Public client (PKCE)** — enabled
   - **Redirect URIs** — `http://127.0.0.1:*/callback`
   - **Scopes** — `openid profile email`
4. Copy the **Client ID** (`pk_test_...` for dev).
5. Find your instance issuer URL under **API Keys** — looks like
   `https://<instance>.clerk.accounts.dev`.

### Local config

Copy `.env.example` to `.env`:

```bash
cp apps/desktop/.env.example apps/desktop/.env
```

Fill in:

```
CLERK_ISSUER=https://your-instance.clerk.accounts.dev
CLERK_CLIENT_ID=pk_test_...
```

### Running

```bash
cargo run -p Zaned
```

On first launch you'll see the **Sign in with Clerk** screen. Click it; the
system browser opens Clerk's hosted login page; sign in or create an
account; the browser tab confirms success and you're returned to the
desktop showing the chart UI.

The refresh token is stored in your OS keychain (macOS Keychain, Windows
Credential Manager, Linux Secret Service); subsequent launches restore the
session silently.

### Signing out

There's no UI button for sign out yet. To force re-auth, wipe the keychain
entry manually:

- macOS: `security delete-generic-password -s zaned -a clerk-refresh`
- Linux: `secret-tool clear service zaned account clerk-refresh`
- Windows: Credential Manager → Generic Credentials → `zaned`

### Debugging

Run with verbose auth logs:

```bash
RUST_LOG=zaned=debug,Zaned=debug cargo run -p Zaned
```

If sign-in fails, check:
- `.env` has correct `CLERK_ISSUER` and `CLERK_CLIENT_ID`.
- Clerk OAuth Application has "Public client (PKCE)" enabled.
- Redirect URI in Clerk dashboard is exactly `http://127.0.0.1:*/callback`
  (the `*` allows any random port — required because we bind to a fresh
  port on each sign-in attempt).
