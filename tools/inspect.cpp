// SPDX-License-Identifier: GPL-3.0-or-later
#include "../engines/xfiles/neodb.h"
#include "../engines/xfiles/script.h"
#include <fstream>
#include <iostream>
#include <iterator>
int main(int argc, char **argv) {
	if (argc != 2) { std::cerr << "Usage: xfiles-inspect XFILES.HDB|XFILES.GAM\n"; return 2; }
	std::ifstream file(argv[1], std::ios::binary);
	if (!file) { std::cerr << "Cannot open database\n"; return 2; }
	std::vector<uint8_t> bytes((std::istreambuf_iterator<char>(file)), {});
	XFiles::NeoDb db;
	if (!db.load(bytes.data(), bytes.size())) { std::cerr << db.error() << '\n'; return 1; }
	std::cout << "app_version=" << db.appVersion() << " neo_version=" << std::hex << db.version() << std::dec
	          << " classes=" << db.classes().size() << '\n';
	for (const auto &cls : db.classes()) {
		std::cout << "class=" << std::hex << cls.id << std::dec << " declared=" << cls.declaredCount
		          << " indexed=" << cls.objects.size() << '\n';
		if (cls.id == 0x35) {
			size_t parsed=0, failed=0;
			for (const auto &entry : cls.objects) {
				XFiles::AssetDescriptor asset;
				if (!db.assetInfo(entry.first,asset)) { ++failed; continue; }
				++parsed;
				for(const auto &variant:asset.entries)
					std::cout << "asset=" << entry.first << " variant=" << variant.variant << " file=" << variant.file
					          << " start=" << variant.start << " end=" << variant.end << '\n';
			}
			std::cout << "asset_blobs_parsed=" << parsed << " other_or_unsupported=" << failed << '\n';
		}
		if(cls.id==0x41){
			size_t good=0,bad=0; std::map<unsigned,size_t> kinds; std::map<std::string,size_t> procedures;
			for(const auto &entry:cls.objects){
				XFiles::Action a;
				if(!XFiles::loadAction(db,entry.first,a)){++bad;continue;}
				++good;++kinds[a.kind()];
				if(a.kind()==6){XFiles::Procedure p;XFiles::parseProcedure(a.body,p);++procedures[p.name];}
			}
			std::cout<<"actions_decoded="<<good<<" unsupported="<<bad<<'\n';
			for(const auto &k:kinds)std::cout<<"action_kind="<<k.first<<" count="<<k.second<<'\n';
			for(const auto &p:procedures)std::cout<<"procedure="<<p.first<<" calls="<<p.second<<'\n';
		}
	}
}
