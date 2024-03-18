<div align="center">
  <h1>Rust ROV Balance System</h1>
  <a href="https://wakatime.com/badge/github/kaganege/rust-rov-balance-system"><img src="https://wakatime.com/badge/github/kaganege/rust-rov-balance-system.svg" alt="wakatime"></a>
</div>

Rust adaptation to [kaganege/rov-balance-system](https://github.com/kaganege/rov-balance-system)

> [!WARNING]
> Not tested!

## Usage

> [!IMPORTANT]
> If you have not installed the thumbv6m-none-eabi target, you need to install it first of all.
>
> ```sh
> rustup target install thumbv6m-none-eabi
> ```

1. Plug the Pico into the computer in boot mode.
2. Compile and upload the code with

    ```sh
    cargo run --release
    ```
