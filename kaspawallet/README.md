# kaspawallet

A single binary covering the entire wallet operator surface: 16 client subcommands for keyfile management, address derivation, balance lookup, transaction construction / signing / broadcast, and fee bumping; plus the `start-daemon` subcommand for running the gRPC wallet daemon in-process.

Build:

```bash
cargo build --release -p kaspawallet
```

The output binary is `target/release/kaspawallet`. Every subcommand below is invoked as `kaspawallet <subcommand> [flags]`.

Network selection is available on every subcommand:

- `--testnet` — testnet (most common operator value).
- `--simnet` — simulation network.
- `--devnet` — development network.
- `--override-dag-params-file <path>` — DAG parameter overrides (devnet only).

The four flags are mutually exclusive; mainnet is the default when none is supplied.

See `kaspawallet --help` for the subcommand list and `kaspawallet <subcommand> --help` for per-subcommand flag details.

---

## Wallet creation

`create` writes an encrypted keyfile to a platform-aware default path (enumerated below under "Single-signature wallet — new mnemonic"). The keyfile holds the BIP-39 mnemonic encrypted under the wallet password supplied at create time.

### Single-signature wallet — new mnemonic (default, Schnorr)

```bash
kaspawallet create
```

Prompts for a wallet password, generates a 24-word BIP-39 mnemonic, derives the master extended key, and writes the keyfile. The mnemonic is printed once; record it offline before the first send.

The keyfile lands at the platform default path under the active network's subdirectory:

- Linux / BSD: `~/.kaspawallet/<network>/keys.json`
- macOS: `~/Library/Application Support/Kaspawallet/<network>/keys.json`
- Windows: `%LOCALAPPDATA%\Kaspawallet\<network>\keys.json`

Override with `--keys-file <path>` (alias `-f`); the override flag is accepted on every subcommand that reads a keyfile (`create`, `dump-unencrypted-data`, `parse`, `send`, `sign`, `bump-fee`).

When neither `--password <password>` nor `--password-file <path>` is supplied, the binary prompts for the password interactively on the TTY. `--password-file` is preferred over `--password <password>` because the literal `--password <password>` form is visible to `ps` and `/proc/<pid>/cmdline`.

**Password-file permissions (MUST).** The password file must be owner-only readable on Unix (mode `0600`). The binary refuses to read a password file with broader permissions.

### Single-signature wallet — restore from mnemonic

```bash
kaspawallet create --import
```

Prompts for a wallet password and then reads the BIP-39 mnemonic from stdin (one line, words space-separated). The keyfile derived from the supplied mnemonic is byte-identical to the original wallet's keyfile (same network + same mnemonic + same scheme → same addresses) and lands at the platform default path (override with `-f <path>`). Invalid mnemonics (wrong checksum, bad word) are rejected with `mnemonic is invalid` before any keyfile is written.

### Single-signature wallet — ECDSA scheme

```bash
kaspawallet create --ecdsa
kaspawallet create --ecdsa --import
```

Selects the ECDSA signing scheme instead of the default Schnorr. ECDSA wallets use different leaf addresses than Schnorr wallets derived from the same mnemonic; an ECDSA wallet created from an existing Schnorr wallet's mnemonic produces a different address set with zero balance. Use `--ecdsa` only when interoperating with an ECDSA-only cosigner or signing device (e.g. a Tangem-class signing card); do not use it as a generic fallback.

### Multi-signature wallet — all keys held locally

```bash
kaspawallet create -m <M> -n <N> -k <N>
```

`-m <M>` is the threshold (minimum required signatures); `-n <N>` is the total cosigner count; `-k <N>` (matching `-n`) declares that the operator holds every cosigner seed locally. For multisig, supply all three with M ≥ 1, M ≤ N, and N ≥ 2; without these flags the binary produces a 1-of-1 single-signature wallet. `-k <N>` is the load-bearing flag for the all-keys-local shape — without it the binary defaults `-k` to 1 and treats the call as cosigner-split with one operator-held seed (see the next subsection). The operator generates all N mnemonics; each cosigner's private key sits in the same keyfile. Suitable for offline-stored backup-grade multisig where one operator controls all seeds. Use `--keys-file <path>` (alias `-f`) to write the keyfile to a non-default location (recommended when keeping multisig wallets separate from a single-signature default keyfile).

### Multi-signature wallet — cosigner-split

```bash
kaspawallet create -m <M> -n <N> -k <K> --import
```

`-k <K>` is the number of private keys the operator holds locally; the remaining `N - K` cosigners contribute extended public keys. Cosigner-split is the default shape whenever `-k < N`, including the case where `-k` is omitted entirely (the binary defaults `-k` to 1, producing a K=1 cosigner-split with one operator-held seed and `N - 1` external xpub prompts). Supply `-k` explicitly with `1 ≤ K < N` when the operator holds more than one seed; omit `-k` for the K=1 shape. The command prompts for the operator's `K` mnemonics on stdin (one per line), then for the `N - K` external cosigner xpubs (one per line). The keyfile records the operator's encrypted mnemonics plus the external xpubs; no external private key material ever enters the keyfile.

**Worked example — 2-of-4 multisig (operator holds 1 key, 3 external xpubs):**

```bash
kaspawallet create --testnet -f wallet-2of4.json -m 2 -n 4 -k 1 --import
# stdin (one line per prompt):
#   <operator's 24-word mnemonic>
#   <external-cosigner-xpub-1>
#   <external-cosigner-xpub-2>
#   <external-cosigner-xpub-3>
```

**Worked example — 3-of-6 multisig (operator holds 2 keys, 4 external xpubs):**

```bash
kaspawallet create --testnet -f wallet-3of6.json -m 3 -n 6 -k 2 --import
# stdin:
#   <operator's 1st 24-word mnemonic>
#   <operator's 2nd 24-word mnemonic>
#   <external-cosigner-xpub-1>
#   <external-cosigner-xpub-2>
#   <external-cosigner-xpub-3>
#   <external-cosigner-xpub-4>
```

ECDSA-scheme multisig is selected with `--ecdsa` on the same command.

### `create` flag reference

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--keys-file` | `-f` | `<path>` | optional (default per platform) | Keyfile location override. |
| `--password` | `-p` | `<password>` | optional | Literal password; visible to `ps`. |
| `--password-file` | — | `<path>` | optional | Password file (mode `0600`); preferred over `--password`. |
| `--yes` | `-y` | — | optional | Assume "yes" to all interactive prompts (overwrite confirmation, etc.). |
| `--min-signatures` | `-m` | `<M>` | optional (default 1) | Multisig threshold. M ≥ 1, M ≤ N. |
| `--num-private-keys` | `-k` | `<K>` | optional (default 1, cosigner-split with K=1 when omitted) | Number of private keys the operator holds locally. Set `-k <N>` (matching `--num-public-keys`) for an all-keys-local multisig where the operator owns every cosigner seed. Set `-k` with `1 ≤ K < N` for a K-seed cosigner-split. Omitting `-k` lands a K=1 cosigner-split with `N - 1` external xpub prompts. |
| `--num-public-keys` | `-n` | `<N>` | optional (default 1) | Total cosigner count. N ≥ M. |
| `--ecdsa` | — | — | optional | Create an ECDSA wallet instead of the default Schnorr. |
| `--import` | `-i` | — | optional | Import existing private keys (mnemonic from stdin) instead of generating new ones. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |


---

## Daily operations against a running daemon

The CLI subcommands below connect to a `kaspawallet start-daemon` instance over gRPC. Default connection target is `localhost:8082`; override with `--daemonaddress <host:port>` (alias `-d`).

### Balance and addresses

```bash
kaspawallet balance
kaspawallet balance --verbose
```

Plain form reports total balance; `--verbose` (alias `-v`) breaks down balance per derivation address.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--verbose` | `-v` | — | optional | Per-address balance breakdown. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

```bash
kaspawallet show-addresses
kaspawallet new-address
```

`show-addresses` lists every derivation address the daemon has generated. `new-address` advances the derivation index and prints the next external receiving address. Both subcommands take the same flag set:

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

### Sending

```bash
kaspawallet send \
    --to-address <kaspa:...> \
    --send-amount <KAS> \
    --password-file <password-file>
```

Constructs an unsigned transaction inside the daemon, signs it with the keyfile's private key(s), and broadcasts via `kaspad`. Returns the broadcast transaction ID.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--to-address` | `-t` | `<kaspa:...>` | yes | Destination address. |
| `--send-amount` | `-v` | `<KAS>` | one of `--send-amount` / `--send-all` | Amount in KAS. |
| `--send-all` | — | — | one of `--send-amount` / `--send-all` | Drain the wallet (mutually exclusive with `--send-amount`). |
| `--from-address` | `-a` | `<kaspa:...>` | optional (repeat) | Restrict input selection to specific source addresses. |
| `--use-existing-change-address` | `-u` | — | optional | Reuse the prior change address rather than minting a new one. |
| `--max-fee-rate` | `-m` | `<sompi/gram>` | optional | Cap the fee rate the wallet uses. |
| `--fee-rate` | `-r` | `<sompi/gram>` | optional | Override the fee-rate estimate from the connected node. |
| `--max-fee` | `-x` | `<sompi>` | optional | Cap the absolute fee. |
| `--show-serialized` | `-s` | — | optional | Also emit hex-encoded signed transactions. |
| `--keys-file` | `-f` | `<path>` | optional (default per platform) | Keyfile override. |
| `--password` | `-p` | `<password>` | optional | Literal password; visible to `ps` / `/proc/<pid>/cmdline`. |
| `--password-file` | — | `<path>` | optional | Password file (mode `0600`); preferred over `--password`. |
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

When neither `--password` nor `--password-file` is supplied, the binary prompts for the password interactively on the TTY.

### Fee bumping (RBF)

```bash
kaspawallet bump-fee --txid <pending-txid> --fee-rate <higher-rate> --password-file <pw-file>
```

Constructs a replacement transaction at a higher fee rate, signs it locally, and broadcasts it through `kaspad`'s replacement-aware submit path.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--txid` | `-i` | `<hex>` | yes | Transaction ID of the pending mempool entry to replace. |
| `--from-address` | `-a` | `<kaspa:...>` | optional (repeat) | Restrict input selection to specific source addresses. |
| `--use-existing-change-address` | `-u` | — | optional | Reuse the prior change address. |
| `--max-fee-rate` | `-m` | `<sompi/gram>` | optional | Cap the fee rate the wallet uses. |
| `--fee-rate` | `-r` | `<sompi/gram>` | optional | Override the fee-rate estimate from the connected node. |
| `--max-fee` | `-x` | `<sompi>` | optional | Cap the absolute fee. |
| `--show-serialized` | `-s` | — | optional | Emit hex-encoded signed replacement transactions. |
| `--keys-file` | `-f` | `<path>` | optional (default per platform) | Keyfile override. |
| `--password` | `-p` | `<password>` | optional | Literal password; visible to `ps`. |
| `--password-file` | — | `<path>` | optional | Password file (mode `0600`); preferred over `--password`. |
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

```bash
kaspawallet bump-fee-unsigned --txid <pending-txid> --fee-rate <higher-rate>
```

Returns the unsigned replacement transaction(s) as hex without signing or broadcasting. Useful when signing happens on a separate offline host.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--txid` | `-i` | `<hex>` | yes | Transaction ID of the pending mempool entry to replace. |
| `--from-address` | `-a` | `<kaspa:...>` | optional (repeat) | Restrict input selection to specific source addresses. |
| `--use-existing-change-address` | `-u` | — | optional | Reuse the prior change address. |
| `--max-fee-rate` | `-m` | `<sompi/gram>` | optional | Cap the fee rate the wallet uses. |
| `--fee-rate` | `-r` | `<sompi/gram>` | optional | Override the fee-rate estimate. |
| `--max-fee` | `-x` | `<sompi>` | optional | Cap the absolute fee. |
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

No keyfile / password flags: the daemon returns unsigned hex; signing happens elsewhere.

### Offline / PSTX signing flow

Construct an unsigned transaction on a host that has access to the daemon, sign it offline on a host that has the keyfile, then broadcast back from a host that can reach the daemon. The signing host need not run the daemon.

```bash
# host A (online):
kaspawallet create-unsigned-transaction \
    --to-address <kaspa:...> \
    --send-amount <KAS> \
    > unsigned.hex

# host B (offline):
kaspawallet sign \
    --transaction-file unsigned.hex \
    --password-file <pw-file> \
    > signed.hex

# host A (online again):
kaspawallet broadcast --transaction-file signed.hex
```

`--transaction <hex>` (alias `-t`) is also accepted in place of `--transaction-file <path>` (alias `-F`) for short payloads passed on the command line.

**Flags — `create-unsigned-transaction`:**

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--to-address` | `-t` | `<kaspa:...>` | yes | Destination address. |
| `--send-amount` | `-v` | `<KAS>` | one of `--send-amount` / `--send-all` | Amount in KAS. |
| `--send-all` | — | — | one of `--send-amount` / `--send-all` | Drain the wallet. |
| `--from-address` | `-a` | `<kaspa:...>` | optional (repeat) | Restrict input selection. |
| `--use-existing-change-address` | `-u` | — | optional | Reuse the prior change address. |
| `--max-fee-rate` | `-m` | `<sompi/gram>` | optional | Cap the fee rate. |
| `--fee-rate` | `-r` | `<sompi/gram>` | optional | Override the fee-rate estimate. |
| `--max-fee` | `-x` | `<sompi>` | optional | Cap the absolute fee. |
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

No keyfile / password flags: signing happens at `sign` time, not here.

**Flags — `sign`:**

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--transaction` | `-t` | `<hex>` | one of `--transaction` / `--transaction-file` | Unsigned transaction(s) as hex. |
| `--transaction-file` | `-F` | `<path>` | one of `--transaction` / `--transaction-file` | File containing unsigned transaction(s) as hex. |
| `--keys-file` | `-f` | `<path>` | optional (default per platform) | Keyfile override. |
| `--password` | `-p` | `<password>` | optional | Literal password; visible to `ps`. |
| `--password-file` | — | `<path>` | optional | Password file (mode `0600`); preferred over `--password`. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

No daemon connection: `sign` reads the keyfile directly and emits the signed hex on stdout.

**Flags — `broadcast`:**

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--transaction` | `-t` | `<hex>` | one of `--transaction` / `--transaction-file` | Signed transaction(s) as hex. |
| `--transaction-file` | `-F` | `<path>` | one of `--transaction` / `--transaction-file` | File containing signed transaction(s) as hex. |
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

No keyfile / password flags: `broadcast` only relays the already-signed bytes through the daemon.

**Flags — `broadcast-replacement`:**

Identical flag set to `broadcast`; the daemon routes the request through `kaspad`'s replacement-aware submit path.

#### Multisig cosigner round-trip

For an M-of-N multisig wallet, every cosigner contributes a partial signature against the same PSTX hex. Each cosigner runs `sign` on its own offline host with its own keyfile; the final cosigner (whoever holds the M-th private key) emits a fully-signed PSTX that any host can broadcast.

```bash
# cosigner 1 (offline, signs the original unsigned PSTX):
kaspawallet sign --transaction-file unsigned.hex --password-file <cosigner-1-pw> > partial-1.hex

# cosigner 2 (offline, signs the partial after cosigner 1):
kaspawallet sign --transaction-file partial-1.hex --password-file <cosigner-2-pw> > partial-2.hex

# … repeat for cosigners 3 … M …
# final cosigner emits a fully-signed PSTX. Broadcast from any online host:
kaspawallet broadcast --transaction-file partial-M.hex
```

For replacement transactions, swap the final `broadcast` for `broadcast-replacement` against the same flag set.

### Parsing transactions

```bash
kaspawallet parse --transaction-file signed.hex
kaspawallet parse --transaction-file signed.hex --verbose
```

Decodes the hex into a human-readable summary (inputs, outputs, fees, masses). With `--verbose` (alias `-v`) the per-input UTXO details are also shown. Passing `--keys-file <path>` (alias `-f`) annotates which outputs are owned by the keyfile's wallet.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--transaction` | `-t` | `<hex>` | one of `--transaction` / `--transaction-file` | Transaction(s) to decode. |
| `--transaction-file` | `-F` | `<path>` | one of `--transaction` / `--transaction-file` | File containing transaction(s) to decode. |
| `--keys-file` | `-f` | `<path>` | optional | Annotates which outputs are owned by the keyfile's wallet. |
| `--verbose` | `-v` | — | optional | Show per-input UTXO details. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

No password flags: `parse` is read-only and never decrypts the keyfile.

---

## Backup and restore

### Reveal the mnemonic

**Warning.** `dump-unencrypted-data` writes the BIP-39 mnemonic and extended private/public keys to **standard output as plaintext**. The mnemonic is the full wallet secret; anyone who reads it controls the funds. Run this command only on a trusted terminal that is not being recorded, screen-shared, or scrolled-back through. Do not pipe the output to a file you don't intend to encrypt immediately, and do not run it inside a terminal multiplexer whose scrollback buffer persists on disk. The command prints a "this prints sensitive material" confirmation prompt by default; clear the terminal (`clear` / `reset`) after recording the mnemonic to remove the secret from the visible scrollback.

```bash
kaspawallet dump-unencrypted-data
```

Reads the keyfile at the platform default path (override with `-f <path>`), prompts for the wallet password, and prints the BIP-39 mnemonic plus extended private/public keys. Record the mnemonic on paper or an air-gapped offline medium.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--keys-file` | `-f` | `<path>` | optional (default per platform) | Keyfile override. |
| `--password` | `-p` | `<password>` | optional | Literal password; visible to `ps`. |
| `--password-file` | — | `<path>` | optional | Password file (mode `0600`); preferred over `--password`. |
| `--yes` | `-y` | — | optional | Skip the "this prints sensitive material" confirmation prompt. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

When neither `--password` nor `--password-file` is supplied, the binary prompts for the password interactively on the TTY.

### Restore on a new host

Install `kaspawallet` on the new host, then re-create the wallet from the saved mnemonic:

```bash
kaspawallet create --import
# stdin: <the 24-word mnemonic recorded above>
```

The new keyfile holds the same wallet as the original — same addresses, same balance, same signing keys. The mnemonic is the only secret needed; the original keyfile is not transferred. For ECDSA wallets, pass `--ecdsa --import` so the new keyfile uses the same scheme as the original.

For multisig wallets, restore each cosigner's local seed via `--import` and re-supply the external cosigner xpubs (recorded alongside the mnemonic at create time).

### Sweep a single-key address into the running wallet

```bash
kaspawallet sweep --private-key <hex-32-byte-secp256k1>
```

Reads a private key from `--private-key` (alias `-k`) or stdin (when omitted), discovers spendable UTXOs at the address derived from that key, and consolidates them into a fresh address from the daemon's wallet. Useful for retiring legacy single-key addresses or claiming faucet payouts.

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--private-key` | `-k` | `<hex>` | optional (stdin fallback) | 32-byte secp256k1 private key as hex; reads from stdin when omitted. |
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

No keyfile / password flags: sweep operates on an external private key, not the daemon's wallet keyfile.

---

## Running a daemon

The `start-daemon` subcommand runs the gRPC wallet daemon in-process. The four deployment scenarios (S0 single-host / S1 trusted-private-network / S2 internet + TLS / S3 mTLS) and the full security posture, flag reference, and operator checklist for non-local deployments live in [`wallet/daemon/README.md`](../wallet/daemon/README.md).

Quick local-only start (S0 default):

```bash
kaspawallet start-daemon \
    --password <password-file> \
    --name <wallet-name> \
    [--rpc-server <kaspad-wRPC-url> | --network-id <id>]
```

**`--password` semantic differs from the client subcommands.** On `start-daemon` the flag takes a **file path** (PathBuf, required); on every other subcommand `--password` takes a **literal string**. Use `--password <path>` here and `--password-file <path>` on the client subcommands.

### `start-daemon` flag reference

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--password` | `-p` | `<path>` | **yes** (file path) | Path to a file containing the wallet password (cleartext). |
| `--name` | `-n` | `<wallet-name>` | optional | Wallet store name; selects an entry inside the `kaspa-wallet-core` local store. |
| `--rpc-server` | `-s` | `<url>` | one of `--rpc-server` / `--network-id` | Private kaspad wRPC URL. |
| `--network-id` | — | `<id>` | one of `--rpc-server` / `--network-id` | Connect via the Public Node Network. |
| `--listen` | `-l` | `<host:port>` | optional (default `127.0.0.1:8082`) | gRPC listen address. Non-loopback values require `--tls-cert`/`--tls-key` or `--insecure`. |
| `--tls-cert` | — | `<path>` | optional (paired with `--tls-key`) | PEM-encoded TLS server certificate. |
| `--tls-key` | — | `<path>` | optional (paired with `--tls-cert`) | PEM-encoded TLS private key. |
| `--client-ca` | — | `<path>` | optional (requires `--tls-cert` + `--tls-key`) | CA for mutual TLS client-cert validation. |
| `--auth-token` | — | `<path>` | optional | File holding a static API token; clients must send `authorization: Bearer <token>`. |
| `--insecure` | — | — | optional | Allow a non-loopback `--listen` without TLS. |
| `--testnet` / `--simnet` / `--devnet` | — | — | optional (mutually exclusive) | Active network selector. |
| `--override-dag-params-file` | — | `<path>` | optional (devnet only) | DAG parameter overrides. |

The TLS / auth flags' deployment scenarios (S0 loopback / S1 trusted-private-network / S2 internet+TLS / S3 mTLS) and the full security posture live in [`wallet/daemon/README.md`](../wallet/daemon/README.md).

---

## Reporting versions

```bash
kaspawallet version             # local binary version
kaspawallet get-daemon-version  # version of the daemon at --daemonaddress
```

**Flags — `version`:** none.

**Flags — `get-daemon-version`:**

| Flag | Short | Value | Required | Description |
|---|---|---|---|---|
| `--daemonaddress` | `-d` | `<host:port>` | optional (default `localhost:8082`) | Non-default daemon. |

No NetworkFlags: the daemon-reported version is network-agnostic.

---

## Default-value summary

- `--keys-file` — `<app-dir>/<network>/keys.json`, where `<app-dir>` is per-OS:
  - Linux / BSD: `~/.kaspawallet/`
  - macOS: `~/Library/Application Support/Kaspawallet/`
  - Windows: `%LOCALAPPDATA%\Kaspawallet\`
- `--daemonaddress` (client subcommands) — `localhost:8082`.
- `--listen` (`start-daemon`) — `127.0.0.1:8082`.
- `--password` (client subcommands) — interactive TTY prompt when neither `--password` nor `--password-file` is supplied. The password file must be mode `0600`.
- `--min-signatures` / `--num-private-keys` / `--num-public-keys` — all default to 1 (single-signature wallet). Override on `create` to produce a multisig wallet.

---

## Related documentation

- gRPC proto: [`wallet/grpc/core/proto/kaspawalletd.proto`](../wallet/grpc/core/proto/kaspawalletd.proto)
- Daemon deployment guide: [`wallet/daemon/README.md`](../wallet/daemon/README.md)
- Wallet subsystem overview: [`wallet/README.md`](../wallet/README.md)
- `kaspa-wallet-core` library: [`wallet/core/`](../wallet/core/)
- gRPC client library: [`wallet/grpc/client/`](../wallet/grpc/client/)
- Kaspa p2p node (kaspad): [`kaspad/README.md`](../kaspad/README.md) — the node binary every daemon connects to via `--rpc-server` or the Public Node Network
