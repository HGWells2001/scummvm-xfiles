// SPDX-License-Identifier: GPL-3.0-or-later
// Reference: Shiloh port/src/media/asset.rs (MIT); see THIRD_PARTY.md.
#ifndef XFILES_ASSET_H
#define XFILES_ASSET_H
#include <cstdint>
#include <string>
#include <vector>
namespace XFiles {
struct AssetEntry {
	uint32_t variant, start, end;
	std::string file;
};
struct AssetDescriptor {
	std::string authoringPath;
	bool hasRange = false;
	uint32_t start = 0, end = 0;
	std::vector<AssetEntry> entries;
};
bool parseAsset(const std::string &blob, uint8_t flags, AssetDescriptor &out);
bool safeAssetPath(const std::string &path);
}
#endif
