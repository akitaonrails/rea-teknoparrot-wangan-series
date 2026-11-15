# Pinned upstream sources built for the packages (sourced by the scripts).
# Commits are full SHA-1s; scripts/fetch-sources.sh checks them.

# OpenParrot64.dll + OpenParrotLoader64.exe for MT5, 5DX+, 6, 6R
# (patches/openparrot: the WMMT6 save and boost.context fixes)
OPENPARROT_REPO=https://github.com/teknogods/OpenParrot.git
OPENPARROT_COMMIT=fc8ba632e1f67c5c340ff88f30efca743e8d38f5

# OpenParrot64.dll + OpenParrotLoader64.exe for MT6RR
# (patches/openparrot-wmmt: first-boot defaults, terminal wait)
OPENPARROT_WMMT_REPO=https://github.com/valkyrieasyraf/OpenParrot_WMMT.git
OPENPARROT_WMMT_COMMIT=a89d064c45bea1f102e1a9c84da0252147e20c9c

# libwal_3dxp.so for MT3, 3DX+ (patches/wal: the launcher's input block)
WAL_REPO=https://github.com/vixen256/WanganArcadeLoader0.5.git
WAL_COMMIT=d3fc4261ee0ce36c343902d18489a64cac077cf7

# pcsx2x6 for Wangan R: the release AppImage is shipped as built upstream
# (packaging/components.pins); this tree is its corresponding source (tag v0.2.22)
PCSX2X6_REPO=https://github.com/PS2Homebrew-arcade/pcsx2x6.git
PCSX2X6_COMMIT=64f063a6351549b0de9c97971636b23360dbcdab
