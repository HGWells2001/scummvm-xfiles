// SPDX-License-Identifier: GPL-3.0-or-later
#include "asset.h"
#include <limits>
namespace XFiles {
namespace {
class Reader {
public:
	explicit Reader(const std::string &data) : b(data), p(0) {}
	bool number(uint32_t &value) {
		if (p >= b.size()) return false;
		const size_t n = uint8_t(b[p++]);
		if (n > b.size()-p) return false;
		const size_t end = p+n;
		value=0;
		// The original uses decimal strings. Permit surrounding spaces, not signs or junk.
		while (p<end && b[p]==' ') ++p;
		bool digit=false;
		while (p<end && b[p]>='0' && b[p]<='9') {
			const unsigned d=b[p++]-'0';
			if (value > (std::numeric_limits<uint32_t>::max()-d)/10) return false;
			value=value*10+d; digit=true;
		}
		while (p<end && b[p]==' ') ++p;
		return p==end && (digit || n==0);
	}
	bool text(std::string &out) {
		uint32_t n;
		if (!number(n) || n>b.size()-p) return false;
		out.assign(b,p,n); p+=n;
		for (char &c:out) if (uint8_t(c)==0x7f || c=='\\') c='/';
		return true;
	}
	const std::string &b;
	size_t p;
};
}
bool safeAssetPath(const std::string &p) {
	if (p.empty() || p.size()>1024 || p.front()=='/' || p.front()=='\\') return false;
	std::string part;
	for (size_t i=0;i<=p.size();++i) {
		if (i==p.size() || p[i]=='/' || p[i]=='\\') {
			if (part.empty() || part=="." || part=="..") return false;
			part.clear();
		} else {
			if (uint8_t(p[i])<32 || p[i]==':' || p[i]=='\0') return false;
			part+=p[i];
		}
	}
	return true;
}
bool parseAsset(const std::string &blob, uint8_t flags, AssetDescriptor &out) {
	out=AssetDescriptor();
	AssetDescriptor parsed;
	Reader r(blob);
	if (!r.text(parsed.authoringPath)) return false;
	parsed.hasRange=(flags&7)==3;
	if (parsed.hasRange && (!r.number(parsed.start) || !r.number(parsed.end))) return false;
	std::string ignored;
	if (!r.text(ignored)) return false;
	while (r.p<blob.size()) {
		if (blob[r.p]=='\0') {
			// End token; only zero padding may follow.
			while (r.p<blob.size()) if (blob[r.p++]!='\0') return false;
			break;
		}
		AssetEntry e;
		if (!r.number(e.variant) || !r.text(e.file) || !r.number(e.start) || !r.number(e.end)) return false;
		if (!safeAssetPath(e.file) || (e.end && e.end<e.start)) return false;
		parsed.entries.push_back(e);
		if (parsed.entries.size()>256) return false;
	}
	out=parsed;
	return true;
}
}
