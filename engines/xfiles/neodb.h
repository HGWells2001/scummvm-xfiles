// SPDX-License-Identifier: GPL-3.0-or-later
// NeoAccess structure reference: Asterber/shiloh, MIT license; see THIRD_PARTY.md.
#ifndef XFILES_NEODB_H
#define XFILES_NEODB_H
#include <cstdint>
#include <vector>
#include <string>
#include <map>
#include <set>
#include "asset.h"

namespace XFiles {
struct NeoClass {
	uint32_t id, declaredCount, indexRoot;
	std::map<uint32_t, uint32_t> objects;
};
class NeoDb {
public:
	bool load(const uint8_t *data, size_t size);
	uint32_t read32(size_t offset) const;
	uint16_t read16(size_t offset) const;
	uint32_t find(uint32_t cls, uint32_t id) const;
	std::string asset(uint32_t id) const;
	bool assetInfo(uint32_t id, AssetDescriptor &out) const;
	bool list(uint32_t cls, uint32_t id, std::vector<uint32_t> &out);
	bool blobAt(size_t reference, std::vector<uint8_t> &out) const;
	bool recordBytes(uint32_t cls, uint32_t id, size_t length, std::vector<uint8_t> &out) const;
	const std::vector<NeoClass> &classes() const { return _classes; }
	const std::string &error() const { return _error; }
	uint32_t appVersion() const { return _appVersion; }
	uint32_t version() const { return _version; }
private:
	std::vector<uint8_t> _data;
	std::vector<NeoClass> _classes;
	std::string _error;
	uint32_t _appVersion = 0, _version = 0;
	bool range(size_t offset, size_t length) const;
	bool fail(const char *message);
	bool leaves(uint32_t mark, unsigned depth, std::set<uint32_t> &seen, std::vector<uint32_t> &out);
};
}
#endif
