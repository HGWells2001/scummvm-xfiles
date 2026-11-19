// SPDX-License-Identifier: GPL-3.0-or-later
// Include the portable parser before ScummVM's forbidden-symbol macros.
#include "neodb.h"
#include "engines/advancedDetector.h"
#include "engines/engine.h"
#include "common/file.h"
#include "common/fs.h"
#include "common/config-manager.h"
#include "common/system.h"
#include "common/translation.h"
#include "gui/message.h"
#include "base/plugins.h"

namespace XFiles {
class XFilesEngine : public Engine {
public:
	explicit XFilesEngine(OSystem *syst) : Engine(syst) {}
	Common::Error run() override {
		Common::FSNode root(ConfMan.getPath("path"));
		SearchMan.addDirectory("xfiles-mininst", root.getChild("MININST"), 0, 2);
		SearchMan.addDirectory("xfiles-medinst", root.getChild("MEDINST"), 0, 2);
		NeoDb title, state;
		Common::String failure;
		if (!load("XFILES.HDB", title, failure) || !load("XFILES.GAM", state, failure))
			return Common::Error(Common::kReadingFailed, failure);
		const Common::String message = Common::String::format(
			"The X-Files: database research prototype\n\n"
			"Title database: %u classes\nGame-state database: %u classes\n\n"
			"The game is NOT playable yet. Scene execution, scripts, dialogue, inventory and saving remain unimplemented.",
			(unsigned)title.classes().size(), (unsigned)state.classes().size());
		GUI::MessageDialog dialog{Common::U32String(message)};
		dialog.runModal();
		return Common::kNoError;
	}
private:
	bool load(const char *name, NeoDb &db, Common::String &failure) {
		Common::File f;
		if (!f.open(name)) { failure = Common::String::format("Missing %s", name); return false; }
		if (f.size() < 24 || f.size() > 128 * 1024 * 1024) { failure = "Invalid database size"; return false; }
		std::vector<uint8_t> bytes((size_t)f.size());
		if (f.read(bytes.data(), (uint32)bytes.size()) != bytes.size()) { failure = "Truncated database"; return false; }
		if (!db.load(bytes.data(), bytes.size())) { failure = db.error().c_str(); return false; }
		return true;
	}
};
}
class XFilesMetaEngine : public AdvancedMetaEngine<ADGameDescription> {
public:
	const char *getName() const override { return "xfiles"; }
	Common::Error createInstance(OSystem *syst, Engine **engine, const ADGameDescription *) const override {
		*engine = new XFiles::XFilesEngine(syst); return Common::kNoError;
	}
};
#if PLUGIN_ENABLED_DYNAMIC(XFILES)
REGISTER_PLUGIN_DYNAMIC(XFILES, PLUGIN_TYPE_ENGINE, XFilesMetaEngine);
#else
REGISTER_PLUGIN_STATIC(XFILES, PLUGIN_TYPE_ENGINE, XFilesMetaEngine);
#endif
