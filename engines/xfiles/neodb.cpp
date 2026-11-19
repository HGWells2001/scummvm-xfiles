// SPDX-License-Identifier: GPL-3.0-or-later
#include "neodb.h"

namespace XFiles {
bool NeoDb::range(size_t p, size_t n) const {
	return p <= _data.size() && n <= _data.size() - p;
}
bool NeoDb::fail(const char *message) { _error = message; return false; }
uint32_t NeoDb::read32(size_t p) const {
	if (!range(p, 4)) return 0;
	return uint32_t(_data[p]) << 24 | uint32_t(_data[p+1]) << 16 |
	       uint32_t(_data[p+2]) << 8 | _data[p+3];
}
uint16_t NeoDb::read16(size_t p) const {
	if (!range(p, 2)) return 0;
	return uint16_t(_data[p]) << 8 | _data[p+1];
}
bool NeoDb::leaves(uint32_t mark, unsigned depth, std::set<uint32_t> &seen, std::vector<uint32_t> &out) {
	if (!mark || !range(mark, 8)) return fail("Invalid NeoAccess tree pointer");
	if (depth > 32 || !seen.insert(mark).second) return fail("Cyclic or excessive NeoAccess tree");
	// Stored flags are bit-reversed per byte. Stored bit 7 is decoded fLeaf.
	if (_data[mark] & 0x80) { out.push_back(mark); return true; }
	const uint32_t n = read16(size_t(mark) + 6);
	if (!n || !range(size_t(mark) + 8, size_t(n) * 4)) return fail("Truncated internal tree node");
	for (uint32_t i = 0; i < n; ++i)
		if (!leaves(read32(size_t(mark) + 8 + i * 4), depth + 1, seen, out)) return false;
	return true;
}
bool NeoDb::load(const uint8_t *data, size_t size) {
	_data.clear(); _classes.clear(); _error.clear(); _appVersion = _version = 0;
	if (!data || size < 24 || size > 128 * 1024 * 1024) return fail("Invalid database size");
	_data.assign(data, data + size);
	_appVersion = read32(0); _version = read32(20);
	if (_appVersion < 1 || _appVersion > 5 || _version != 0x501) return fail("Unsupported NeoAccess version");
	const uint32_t count = read32(12);
	if (!count || count > 4096) return fail("Invalid class count");
	std::set<uint32_t> seen, classIds;
	std::vector<uint32_t> nodes;
	if (!leaves(read32(8), 0, seen, nodes)) return false;
	for (uint32_t leaf : nodes) {
		size_t p = size_t(leaf) + 8;
		const uint32_t n = read16(size_t(leaf) + 6);
		if (n > count) return fail("Invalid class leaf count");
		for (uint32_t i = 0; i < n; ++i) {
			if (!range(p, 27)) return fail("Truncated class descriptor");
			const uint32_t indexes = _data[p + 10];
			if (!range(p + 27, size_t(indexes) * 16)) return fail("Truncated class indexes");
			NeoClass cls{read32(p + 11), read32(p + 15), indexes ? read32(p + 31) : 0, {}};
			if (!classIds.insert(cls.id).second) return fail("Duplicate class descriptor");
			_classes.push_back(cls);
			if (_classes.size() > count) return fail("Too many class descriptors");
			p += 27 + size_t(indexes) * 16;
		}
	}
	if (_classes.size() != count) return fail("Class count does not match header");
	for (NeoClass &cls : _classes) {
		if (!cls.indexRoot) continue;
		nodes.clear(); seen.clear();
		if (!leaves(cls.indexRoot, 0, seen, nodes)) return false;
		for (uint32_t leaf : nodes) {
			const uint32_t n = read16(size_t(leaf) + 6);
			if (!range(size_t(leaf) + 8, size_t(n) * 8)) return fail("Truncated object index");
			for (uint32_t i = 0; i < n; ++i) {
				const size_t p = size_t(leaf) + 8 + i * 8;
				const uint32_t mark = read32(p), id = read32(p + 4);
				if (!mark || !range(mark, 6)) return fail("Invalid object record pointer");
				cls.objects[id] = mark; // Last duplicate wins, preserving index order.
			}
		}
	}
	return true;
}
uint32_t NeoDb::find(uint32_t cls, uint32_t id) const {
	for (const NeoClass &c : _classes) {
		if (c.id != cls) continue;
		auto it = c.objects.find(id);
		return it == c.objects.end() ? 0 : it->second;
	}
	return 0;
}
std::string NeoDb::asset(uint32_t id) const {
	const uint32_t record = find(0x35, id);
	if (!record || !range(record, 14)) return {};
	const uint32_t mark = read32(size_t(record) + 6), len = read32(size_t(record) + 10);
	if (!len || len > 4096 || !range(mark, len)) return {};
	std::string result(reinterpret_cast<const char *>(_data.data() + mark), len);
	while (!result.empty() && result.back() == '\0') result.pop_back();
	return result;
}
bool NeoDb::assetInfo(uint32_t id, AssetDescriptor &out) const {
	out=AssetDescriptor();
	const uint32_t record=find(0x35,id);
	if (!record || !range(record,24)) return false;
	return parseAsset(asset(id),_data[size_t(record)+22],out);
}
bool NeoDb::list(uint32_t cls, uint32_t id, std::vector<uint32_t> &out) {
	out.clear();
	const uint32_t record=find(cls,id);
	if (!record || !range(record,32)) return fail("Missing or truncated persistent list");
	const uint32_t count=read32(size_t(record)+6), root=read32(size_t(record)+10);
	if (!count && !root) return true;
	std::set<uint32_t> seen;
	std::vector<uint32_t> nodes, result;
	if (!leaves(root,0,seen,nodes)) return false;
	for (uint32_t leaf:nodes) {
		const uint32_t n=read16(size_t(leaf)+6);
		if (!range(size_t(leaf)+8,size_t(n)*4) || result.size()+n>count) return fail("Invalid persistent list length");
		for(uint32_t i=0;i<n;++i) result.push_back(read32(size_t(leaf)+8+i*4));
	}
	if(result.size()!=count) return fail("Persistent list count mismatch");
	out.swap(result);
	return true;
}
}
