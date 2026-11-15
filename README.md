# xmip-core-contract-hl7-v2

the HL7 v2 content contract: a sound ER7 message always, of a bound message type and version when a Location names one, and the acknowledgment composed from the message it answers. Every HL7 v2 version lives here. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

A message is read by the Foundation's one segment walk
([xmip-core-message](https://github.com/IlleNilsson/xmip-core-message)'s
`segment`, which the ER7 shape sections by too): `er7::Message::walk` reads
the encoding characters `MSH` declares and hands the walk its separators,
every line break ending a segment and the last ending with the text, and
holds each identifier to three letters or digits. The contract cuts every
segment into its fields (`er7::Message::parse`), and the `hl7-v2` path
technology ([xmip-core-path-hl7-v2](https://github.com/IlleNilsson/xmip-core-path-hl7-v2))
cuts only the field it reads and walks the rest uncut, so a malformed
message is refused in the same words, at the same segment, by both.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
