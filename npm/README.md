# snowbros

Deterministic engineering intelligence for JavaScript/TypeScript codebases.

This package is a thin wrapper that downloads the prebuilt SNOWBROS
Inspector binary for your platform (SHA-256 verified) and exposes it as
`sb` and `snowbros`.

```sh
npx snowbros analyze
```

Supported platforms: Windows x64, macOS x64/arm64, Linux x64/arm64.
On other platforms, build from source: `cargo install snowbros --locked`.

Full documentation:
[github.com/snowbros/snowbros-inspector](https://github.com/snowbros/snowbros-inspector)
