#!/bin/bash

# This script is an example of how to sign a timestamp for the DMS.
# The signed file is provided on server "foo" in the file "/var/www/example.net/dms",
# which is then served by "foo" over "https://example.net/dms".

host="${1:-foo}"
path="${2:-/var/www/example.net/dms}"

# shellcheck disable=SC2029
date -Is | gpg --clearsign | ssh "${host}" tee "${path}"
