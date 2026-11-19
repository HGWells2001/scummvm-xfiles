// SPDX-License-Identifier: GPL-3.0-or-later
#include "../engines/xfiles/asset.h"
#include <iostream>
#include <cstdlib>
static void check(bool b,const char *n){if(!b){std::cerr<<"FAIL "<<n<<'\n';std::exit(1);}}
static std::string number(uint32_t n){std::string s=std::to_string(n);return char(s.size())+s;}
static std::string text(const std::string &s){return number((uint32_t)s.size())+s;}
static std::string descriptor(const std::string &file,uint32_t variant=1){
	return text("Synthetic/source.mov")+text("")+number(variant)+text(file)+number(0)+number(0);
}
int main(){
	XFiles::AssetDescriptor d;
	auto b=descriptor("XV/12345.xmv",0);
	check(XFiles::parseAsset(b,1,d),"file descriptor");
	check(d.entries.size()==1 && d.entries[0].variant==0 && d.entries[0].file=="XV/12345.xmv","zero variant");
	check(XFiles::parseAsset(descriptor("XV\x7f" "12345.xmv"),1,d) && d.entries[0].file=="XV/12345.xmv","DEL separator");
	check(XFiles::parseAsset(descriptor("XV\\12345.xmv"),1,d) && d.entries[0].file=="XV/12345.xmv","backslash separator");
	std::string timed=text("Synthetic/nav.mov")+number(7000)+number(7090)+text("")+number(1)+text("XN/12345.xmv")+number(7000)+number(7090);
	check(XFiles::parseAsset(timed,3,d) && d.hasRange && d.start==7000 && d.entries[0].end==7090,"nav range");
	for(size_t i=0;i<timed.size();++i) {
		bool ok=XFiles::parseAsset(timed.substr(0,i),3,d);
		// A prefix ending before the optional entry list is a valid empty descriptor.
		if(ok) check(d.entries.empty(),"truncation never yields partial entries");
	}
	for(const char *p:{"../evil.xmv","XV/../evil.xmv","/etc/passwd","C:/game.xmv","//server/file","XV//a","XV/./a"})
		check(!XFiles::parseAsset(descriptor(p),1,d),"unsafe path rejected");
	check(!XFiles::parseAsset(std::string("\x0a" "4294967296"),1,d),"decimal overflow");
	check(!XFiles::parseAsset(std::string("\x02" "-1"),1,d),"negative size");
	check(XFiles::parseAsset(b+std::string(3,'\0'),1,d),"zero padding");
	check(!XFiles::parseAsset(b+std::string(1,'\0')+"x",1,d) && d.entries.empty(),"nonzero trailer and reset");
	std::cout<<"PASS: file variants, ranges, separators, truncation, unsafe paths, overflow and padding\n";
}
