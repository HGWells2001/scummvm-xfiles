// SPDX-License-Identifier: GPL-3.0-or-later
#include "../engines/xfiles/neodb.h"
#include <iostream>
#include <cstdlib>
static void put32(std::vector<uint8_t> &b, size_t p, uint32_t v) {
	for (unsigned i=0; i<4; ++i) b[p+i] = uint8_t(v >> (24-8*i));
}
static void require(bool value, const char *name) {
	if (!value) { std::cerr << "FAIL: " << name << '\n'; std::exit(1); }
}
static std::vector<uint8_t> fixture() {
	std::vector<uint8_t> b(640, 0);
	put32(b,0,5); put32(b,8,64); put32(b,12,1); put32(b,20,0x501);
	b[64]=0x80; b[71]=1; // class-tree leaf with one descriptor
	b[82]=1; put32(b,83,0x35); put32(b,87,1); put32(b,103,256);
	b[256]=0x80; b[263]=1; put32(b,264,512); put32(b,268,42);
	put32(b,518,600); put32(b,522,4);
	b[600]='t'; b[601]='e'; b[602]='s'; b[603]='t';
	return b;
}
int main() {
	XFiles::NeoDb db; auto b=fixture();
	require(db.load(b.data(),b.size()),"valid database");
	require(db.classes().size()==1 && db.find(0x35,42)==512,"object index");
	require(db.asset(42)=="test" && db.asset(43).empty(),"bounded asset blob");
	// Every prefix that cuts the object header must fail, rather than dereference out of range.
	for(size_t n=0;n<518;++n) require(!db.load(b.data(),n),"truncated database");
	auto x=b; x[64]=0; put32(x,72,64);
	require(!db.load(x.data(),x.size()),"class-tree cycle");
	x=b; put32(x,103,0xffffffff);
	require(!db.load(x.data(),x.size()),"out-of-range index");
	x=b; put32(x,264,0xffffffff);
	require(!db.load(x.data(),x.size()),"out-of-range object");
	x=b; put32(x,12,2);
	require(!db.load(x.data(),x.size()),"inconsistent class count");
	x=b; put32(x,518,0xffffffff); require(db.load(x.data(),x.size()),"structurally valid bad blob");
	require(db.asset(42).empty(),"bad blob rejected");
	x=b; put32(x,20,0x999); require(!db.load(x.data(),x.size()),"unsupported version");
	require(db.load(b.data(),b.size()) && db.asset(42)=="test","reload after failure");
	std::cout << "PASS: valid indexes, blobs, truncation, cycles, bounds, version and reload\n";
}
