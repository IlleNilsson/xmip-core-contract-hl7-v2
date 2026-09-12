# xmip-core-contract-hl7-v2

the HL7 v2 content contract: a sound ER7 message always, of a bound message type and version when a Location names one, and the acknowledgment composed from the message it answers. Every HL7 v2 version lives here. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
