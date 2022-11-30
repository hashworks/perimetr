use std::string::FromUtf8Error;

pub(crate) fn null_terminated_bytes_to_string(bytes: &[u8]) -> Result<String, FromUtf8Error> {
    String::from_utf8(bytes.iter().take_while(|&&b| b != 0).map(|b| *b).collect())
}
