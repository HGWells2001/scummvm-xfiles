// SPDX-License-Identifier: GPL-3.0-or-later
#include "../engines/xfiles/neodb.h"
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
		if (cls.id == 0x35) for (const auto &entry : cls.objects)
			std::cout << "asset=" << entry.first << " path=" << db.asset(entry.first) << '\n';
	}
}
