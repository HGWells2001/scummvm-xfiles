// SPDX-License-Identifier: GPL-3.0-or-later
#include "engines/advancedDetector.h"
#include "base/plugins.h"

static const PlainGameDescriptor games[] = {
	{"xfiles", "The X-Files Game (1998)"}, {nullptr, nullptr}
};
#ifdef USE_XFILES_SHILOH
static const char *const variant="Experimental in-process Shiloh runtime";
#else
static const char *const variant="Database research prototype (not playable)";
#endif
static const ADGameDescription descriptions[] = {
	{"xfiles", variant,
	 AD_ENTRY2s("XFILES.HDB", "a87bbe0f3b51856303386db083c0321b", 6042472,
	            "XFILES.GAM", "1de1e4d2a05c5185605232caee341661", 154656),
	 Common::UNK_LANG, Common::kPlatformWindows, ADGF_UNSTABLE, GUIO1(GUIO_NOMIDI)},
	AD_TABLE_END_MARKER
};
static const char *const globs[] = {"MININST", "MEDINST", nullptr};
class XFilesDetection : public AdvancedMetaEngineDetection<ADGameDescription> {
public:
	XFilesDetection() : AdvancedMetaEngineDetection(descriptions, games) {
		_maxScanDepth = 2; _directoryGlobs = globs;
	}
	const char *getName() const override { return "xfiles"; }
	const char *getEngineName() const override { return "The X-Files (experimental)"; }
	const char *getOriginalCopyright() const override { return "(C) 1998 HyperBole Studios / Fox Interactive"; }
};
REGISTER_PLUGIN_STATIC(XFILES_DETECTION, PLUGIN_TYPE_ENGINE_DETECTION, XFilesDetection);
