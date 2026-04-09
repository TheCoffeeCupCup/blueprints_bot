// Copy this file into secrets.rs and replace placeholders with real values before building the executable.

use crate::secret;

////////////////////////////////////////////////////
/* START - ONLY CHANGE THINGS INSIDE THIS SECTION */
////////////////////////////////////////////////////

// See https://discord.com/developers/applications.
secret!(discord_token, "", required);

// RMB on the server -> Copy Server ID.
secret!(guild_id, "", required);

// Any passphrase for bot data encryption.
secret!(bot_data_key, "", required);
// In case the passphrase has changed, you can put the old one here to still be able to load the old data.
secret!(last_bot_data_key, "");

//////////////////////////////////////////////////
/* END - ONLY CHANGE THINGS INSIDE THIS SECTION */
//////////////////////////////////////////////////

#[macro_export]
macro_rules! secret {
    ($name:ident, $value:expr) => {
        pub fn $name() -> String {
            cryptify::encrypt_string!($value)
        }
    };

    ($name:ident, $value:expr, required) => {
        // A hack for compile time checks. Throws compilation error in case of an empty string.
        const _: () = {
            if $value.as_bytes().is_empty() {
                panic!(concat!(
                    "Secret `",
                    stringify!($name),
                    "` is required but is empty!"
                ));
            }
        };

        $crate::secret!($name, $value);
    };
}
