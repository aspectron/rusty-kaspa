# kaspa-wallet-daemon (`kaspawalletd`)

A Rust-native gRPC wallet daemon. Hosts a long-running [`kaspa-wallet-core`](../core/) wallet instance and exposes the [`kaspawalletd.proto`](../grpc/core/proto/kaspawalletd.proto) interface (12 RPCs) to local and remote clients.

Build:

```bash
cargo build --release --package kaspa-wallet-daemon --bin kaspa-wallet-daemon
```

Run (local-only default):

```bash
kaspa-wallet-daemon \
    --password <password-file> \
    --name <wallet-name> \
    [--rpc-server <kaspad-wRPC-url> | --network-id <id>]
```

See `kaspa-wallet-daemon --help` for the full flag set.

---

## Security posture

The daemon serves a wallet capable of signing and broadcasting transactions. Anyone who can reach it on the network can, with sufficient credentials, drain the wallet. The defaults are conservative; production deployments are explicit about which network surface they expose and which credentials gate access.

### Default posture — loopback only, plaintext

Unconfigured, the daemon listens on `127.0.0.1:8082` over plaintext gRPC and runs no authentication interceptor. Every RPC reaches `kaspa-wallet-core` directly; the per-RPC `password` proto field on `SendRequest` / `SignRequest` / `BumpFeeRequest` is the only thing protecting the wallet secret on operations that need it.

This default is **safe for single-machine use** where the operator runs the daemon and the client (`kaspawallet` or `kaspa-cli`) on the same host. It is **not safe** for any deployment that binds a non-loopback address; the daemon refuses to start in that configuration unless the operator explicitly opts in.

### Threat model

The security model addresses three concerns and **does not** address one:

| Concern | Mitigation | Flag(s) |
|---|---|---|
| Wire confidentiality (RPC traffic readable on the network) | Server TLS encrypts every RPC and response | `--tls-cert`, `--tls-key` |
| Server authentication (client connects to the wrong daemon and sends a `Send` RPC there) | TLS server certificate proves daemon identity to client | same as above |
| Client authentication (unauthorized caller invokes RPCs against the daemon) | Static bearer-token interceptor rejects unauthenticated requests, optionally combined with mTLS for cryptographic client identity | `--auth-token`; optional `--client-ca` |
| Wallet-secret disclosure (an authenticated caller still cannot sign without the wallet password) | Per-RPC `password` field on `SendRequest` / `SignRequest` / `BumpFeeRequest` is the user's mnemonic-decryption secret; the daemon never persists it across calls | proto-level, not a daemon flag |

The daemon's network-level auth (TLS + bearer token) and the per-RPC `password` are **two independent layers**. A leaked bearer token lets an attacker enumerate addresses and balances, but cannot sign or send without the wallet password too. A leaked wallet password without a bearer token does nothing because the attacker cannot reach the RPC surface.

What this model **does not** address (out of scope, separate concerns):

- DoS protection against authenticated callers (no per-token rate limiting).
- Side-channel leakage from timing of RPC responses.
- Long-term key material on disk (the wallet keys file itself; encryption-at-rest is `kaspa-wallet-core`'s responsibility).
- Daemon-local privilege escalation (operating-system file-permissions on the keys file and the token file).

### Configuration flags

The TLS and authentication flags are listed below in the order an operator typically considers them.

#### `--listen <host:port>`

Default `127.0.0.1:8082`. The address the gRPC server binds to. A non-loopback value (`0.0.0.0`, a routable external IP, an LAN address) requires either `--tls-cert` + `--tls-key` OR `--insecure`; the daemon refuses to start otherwise. This guard prevents accidental network exposure of an unauthenticated plaintext daemon.

#### `--tls-cert <path>` and `--tls-key <path>`

Server TLS. Both flags are required together — supplying only one is a startup error rather than silent plaintext degradation. The cert may be a self-signed certificate (for closed deployments) or a CA-issued certificate (for open internet exposure). When TLS is enabled the operator-facing startup log line names the scheme as `https`.

If `--tls-key` is encrypted, decrypt it before pointing the daemon at it; the daemon does not prompt for a key passphrase.

#### `--client-ca <path>` (optional mTLS)

Mutual TLS. The daemon validates client certificates against the named CA. Clients without a valid cert from this CA are rejected at the TLS handshake before any RPC reaches the application layer. Requires `--tls-cert` and `--tls-key`. Use mTLS when the operator controls both ends and wants cryptographic client identity rather than a shared-secret bearer token.

#### `--auth-token <path>`

Static bearer-token authentication. The daemon reads the token from the named file (trailing whitespace trimmed so a normal text file with a trailing newline still matches). On every RPC the daemon's tonic interceptor extracts the `authorization` request metadata, expects `Bearer <token>` verbatim, and rejects mismatches. The interceptor applies to every method including `Shutdown` — there is no anonymous-shutdown path.

An empty token file is a startup error: empty means everyone passes auth, which is identical to not having auth at all and is almost always a misconfiguration.

#### `--insecure`

Explicit escape hatch. When set, the daemon binds a non-loopback address without TLS. Use only on a trusted private network (VPN, isolated LAN) where confidentiality is provided by the network layer. The startup log line names this configuration as `http` so misconfigurations surface immediately.

### Recommended deployment shapes

Pick the row that matches your operator scenario.

#### S0 — Single host (default)

Daemon and clients on the same machine. No flags beyond the wallet setup.

```bash
kaspa-wallet-daemon --password <password-file> --name <wallet>
```

Listens on `127.0.0.1:8082`. Only local processes can reach the daemon. Suitable for personal wallets, scripted local automation, and CI smoke tests.

#### S1 — Remote on a trusted private network (e.g. VPN, ZeroTier, Tailscale)

The network layer provides confidentiality; the daemon adds caller authentication.

```bash
kaspa-wallet-daemon \
    --password <password-file> \
    --name <wallet> \
    --listen 0.0.0.0:8082 \
    --auth-token <token-file> \
    --insecure
```

Generate the token (any 32+ byte random secret works):

```bash
openssl rand -hex 32 > /etc/kaspawalletd/auth.token
chmod 600 /etc/kaspawalletd/auth.token
```

Share the same token file with each authorized client. Rotating the token is a daemon restart.

`--insecure` is required because the daemon binds a non-loopback address without TLS. The network-layer confidentiality assumption is the operator's responsibility — this is not a default to choose lightly.

#### S2 — Remote on an untrusted network (e.g. internet)

TLS encrypts the wire; the bearer token authenticates callers.

```bash
kaspa-wallet-daemon \
    --password <password-file> \
    --name <wallet> \
    --listen 0.0.0.0:8082 \
    --tls-cert /etc/kaspawalletd/cert.pem \
    --tls-key /etc/kaspawalletd/key.pem \
    --auth-token /etc/kaspawalletd/auth.token
```

The certificate can be issued by Let's Encrypt or any CA the clients trust. For closed deployments a self-signed certificate plus client-side CA pinning is acceptable.

#### S3 — Remote with cryptographic client identity (mTLS)

Strongest model. Both ends are authenticated by certificate; the bearer token is optional (mTLS proves client identity by itself, but defense-in-depth keeps the token too).

```bash
kaspa-wallet-daemon \
    --password <password-file> \
    --name <wallet> \
    --listen 0.0.0.0:8082 \
    --tls-cert /etc/kaspawalletd/cert.pem \
    --tls-key /etc/kaspawalletd/key.pem \
    --client-ca /etc/kaspawalletd/client-ca.pem \
    [--auth-token /etc/kaspawalletd/auth.token]
```

Issue client certificates against a private CA; revoke individual clients by removing them from the CA's revocation list and rotating the daemon. Use S3 when the operator runs an organization-wide kaspawalletd fleet and wants per-operator audit trails.

### Operator checklist for non-local deployment

Before exposing the daemon beyond loopback:

- [ ] Generate or obtain a TLS certificate. For Let's Encrypt: standard ACME flow against the daemon's hostname. For self-signed: `openssl req -newkey rsa:4096 -x509 -days 365 ...`. The cert chain file points at PEM-encoded leaf + intermediates; the key file points at the matching PEM private key.
- [ ] Generate an `--auth-token` file with at least 32 bytes of entropy. Set file permissions to `600`. Distribute the token to authorized clients via an out-of-band channel (encrypted email, password manager, SSH).
- [ ] Decide whether `--client-ca` mTLS is needed. Use it when client identity matters per-operation (audit, revocation). Skip it for single-operator deployments where the token alone is sufficient.
- [ ] Choose the listen address narrowly. `0.0.0.0:port` binds every interface; an explicit `<external-ip>:port` binds only the named interface. Prefer the narrower form when the host has multiple network interfaces.
- [ ] Verify the startup log line — it names the scheme (`http` or `https`) and the auth posture (`with static-token auth` or `without auth`). Misconfigurations show up here before any client connects.
- [ ] Verify a client connection. Use `grpcurl` with the same CA + token to exercise `GetVersion`:
  ```bash
  grpcurl -insecure \
      -H "authorization: Bearer $(cat /etc/kaspawalletd/auth.token)" \
      <daemon-host>:8082 \
      kaspawalletd.kaspawalletd/GetVersion
  ```

### Per-RPC `password` field — what it does and does not do

`SendRequest`, `SignRequest`, and `BumpFeeRequest` each carry a `password` proto field. This is the **wallet secret** used to decrypt the mnemonic for signing. It is **not** the daemon authentication. Three load-bearing facts:

1. The password travels inside the RPC payload, not in the gRPC metadata. Without TLS the password is visible on the wire to any eavesdropper. **Plaintext daemons over a non-loopback address are categorically unsafe for `Send` / `Sign` / `BumpFee` regardless of bearer-token configuration.**
2. The daemon does not cache the password across calls. Each signing RPC carries its own password; the daemon decrypts the mnemonic, signs, zeroizes the secret, and returns.
3. An attacker who reaches the RPC surface (defeats the daemon-level auth) but does not know the wallet password can still call `GetBalance` / `ShowAddresses` / `NewAddress` / `GetVersion` / etc. — these RPCs do not require the wallet secret. They expose privacy information (balances, addresses) but cannot move funds.

The defense-in-depth pattern is therefore: TLS prevents the password from leaking on the wire; bearer token prevents unauthorized callers from spinning up signing or broadcast operations; the wallet password is the final barrier preventing a fully-authenticated caller from moving funds without the operator's authorization. All three layers should be in place for any non-local deployment.

### Threat model summary

The deployment posture above addresses:

- **Eavesdropping on the network** — TLS encrypts every byte the daemon sends or receives.
- **Server impersonation** — the TLS certificate (verified by the client against a CA, pinned, or otherwise validated) prevents an attacker from running a fake `kaspawalletd` and capturing wallet passwords.
- **Unauthorized RPC access** — bearer tokens (or mTLS client certs) gate every RPC including the read-only ones.
- **Accidental exposure** — the non-loopback / no-TLS guard refuses to start; the operator must explicitly choose `--insecure`.

What the model leaves to the operator:

- Keeping the wallet keys file (`~/.kaspa-wallet/<network>/<wallet>.keys`) safe via filesystem permissions and disk encryption.
- Keeping the password file safe (it is the wallet decryption secret).
- Keeping the auth token file safe (it is the RPC authorization secret).
- Distributing tokens, certificates, and CAs to authorized clients through trusted channels.
- Rotating tokens and certificates on a schedule appropriate for the deployment.

---

## Related documentation

- gRPC proto: [`wallet/grpc/core/proto/kaspawalletd.proto`](../grpc/core/proto/kaspawalletd.proto)
- Server library: [`wallet/grpc/server/`](../grpc/server/)
- Client library: [`wallet/grpc/client/`](../grpc/client/)
- `kaspa-wallet-core` reference: [`wallet/core/`](../core/)
- CLI client (when present): [`kaspawallet/`](../../kaspawallet/) — the Go-wallet-compatible CLI binary that fronts this daemon.
