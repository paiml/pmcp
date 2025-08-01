#\!/bin/bash

# Fix ServerCapabilities imports in examples

# For files using pmcp::{..., ServerCapabilities}
find examples -name "*.rs" -exec grep -l "use pmcp::{.*ServerCapabilities" {} \; | while read f; do
    echo "Fixing import in $f"
    sed -i 's/use pmcp::{Server, ServerCapabilities/use pmcp::{Server/g' "$f"
    sed -i 's/use pmcp::{/use pmcp::{types::capabilities::ServerCapabilities, /g' "$f"
done

# For files using pmcp::server::{..., ServerCapabilities}
find examples -name "*.rs" -exec grep -l "use pmcp::server::{.*ServerCapabilities" {} \; | while read f; do
    echo "Fixing server import in $f"
    sed -i 's/, ServerCapabilities//g' "$f"
    # Add the correct import after the server import line
    sed -i '/use pmcp::server::{/a use pmcp::types::capabilities::ServerCapabilities;' "$f"
done

# Fix standalone imports
find examples -name "*.rs" -exec grep -l "^use pmcp::ServerCapabilities" {} \; | while read f; do
    echo "Fixing standalone import in $f"
    sed -i 's/use pmcp::ServerCapabilities/use pmcp::types::capabilities::ServerCapabilities/g' "$f"
done
