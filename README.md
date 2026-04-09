# CCS blueprints bot
Discord bot for easy uploading blueprint files to dedicated Satisfactory servers.

## How to build
- If you don't have **Rust, install** it following the instructions at https://rust-lang.org/tools/install.
- **Download the source code** repository or clone it with [Git](https://git-scm.com/install): `git clone https://github.com/TheCoffeeCupCup/css_installer`.
- Copy `src\secrets.example.rs` into `src\secrets.rs` and replace the placeholder values with the real ones.
- From the project root, **run** `cargo build --release` (or `cargo run --release` to run the bot once the executable is built).

## How to run
- Use `cargo run --release` to build and run the code.
    - If you just want to run the built executable, you can do so from `target\release\ccs_discord_bot`.

## Features
- Allows admins to set up the servers and uploaders one time, afterwards server members with corresponding permissions can upload the files all they want without disturbing anyone.
- Command `/add_server` allows to add a server by specifying name, IP, FTP credentials and session name. The permissions to run this command must be managed from Discord's server settings. By default only admins can run the command.
    - IP is verified with a regex and a clear error message will be returned in case the IP format is wrong.
    - The bot will try to connect to the server and won't add it to the internal list in case of failure.
- Command `/edit_server_uploaders` allows to specify which users or roles can upload files to a specfic server. The permissions to run this command must be managed from Discord's server settings. By default only admins can run the command.
- Command `/upload_blueprints` allows to upload blueprint files to servers. There are no default restrictions as to who can run this command. However the files can only be uploaded to the servers where the user is added as "uploader".
    - Allows to upload 2-10 files (1-5 blueprints in pairs of `.sbp` and `.sbpcfg` files). 10 files is Discord's limitation that doesn't depend on a bot.
    - Every file is individually verified by name and a clear error is returned in case of any issues.
    - If any problems occur while uploading the files, they will be reported in the response message.
- Colored stdout logs and file logs in `logs` folder (automatically created in runtime).
- Automatic backing up of the bot data for easy recovery in case of errors. If needed, must be restored before the next launch of the bot.
