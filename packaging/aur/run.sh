#!/bin/bash

set -e

# Make a copy so we never alter the original
cp -r /pkg /tmp/pkg
cd /tmp/pkg

# Sync database
if [ -n "$SYNC_DATABASE" ]; then
    paru -S --refresh
fi

# Do the actual building. Paru will fetch all dependencies for us (including
# AUR dependencies) and then build the package.
paru -U --noconfirm

# Store the built package(s). Ensure permissions match the original PKGBUILD.
if [ -n "$EXPORT_PKG" ]; then
    sudo chown "$(stat -c '%u:%g' /pkg/PKGBUILD)" ./*pkg.tar*
    sudo mv ./*pkg.tar* /pkg
fi
# Export .SRCINFO for built package
if [ -n "$EXPORT_SRC" ]; then
    makepkg --printsrcinfo > .SRCINFO
    sudo chown "$(stat -c '%u:%g' /pkg/PKGBUILD)" ./.SRCINFO
    sudo mv ./.SRCINFO /pkg
fi
