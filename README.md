# xmip-core-contract-hl7-v2

the HL7 v2 content contract: a sound ER7 message always, of a bound message type and version when a Location names one, and the acknowledgment composed from the message it answers. Every HL7 v2 version lives here. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

A message is read by the Foundation's one segment walk
([xmip-core-message](https://github.com/IlleNilsson/xmip-core-message)'s
`segment`, which the ER7 shape sections by too): `er7::Message::walk` reads
the encoding characters `MSH` declares by the ER7 shape's one reader
([xmip-core-message-hl7-er7](https://github.com/IlleNilsson/xmip-core-message-hl7-er7)'s
`encoding`, the shape of the same standard and the one technology this
contract depends on, ADR-0044, amendment 2026-11-15) and hands the walk its
separators,
every line break ending a segment and the last ending with the text, and
holds each identifier to three letters or digits. The contract cuts every
segment into its fields (`er7::Message::parse`), and the `hl7-v2` path
technology ([xmip-core-path-hl7-v2](https://github.com/IlleNilsson/xmip-core-path-hl7-v2))
cuts only the field it reads and walks the rest uncut, so a malformed
message is refused in the same words, at the same segment, by both.

## The declaration

What a Location naming this contract is configured with is two `const`s in
its `declaration` module, `src/declaration.rs`, beside the code, which its
`ContractFactory` reads (ADR-0064, amendment 2026-11-26):
`declaration::SETTINGS`, `reference`, the message type a message carries,
and `NAME`, `hl7-v2`, the name a Validate step's `format` names it by. Built
with its `declaration` feature alone (`--no-default-features --features
declaration`), the crate is those and nothing more: its code and every crate
the code is built on are behind the default feature. So the library the
surfaces load can carry it without the code, for the language server and the
desktop to suggest and check a Location's contract settings though no node
in their process loaded the technology, once the gathering crate of open
problem 48 links it. `test/Declaration.Test.ps1` in the estate holds that
the declaration builds nothing the contract capability does not.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
