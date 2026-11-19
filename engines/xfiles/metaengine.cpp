// SPDX-License-Identifier: GPL-3.0-or-later
#include "xfiles.h"
#include "engines/advancedDetector.h"
#include "base/plugins.h"
class XFilesMetaEngine : public AdvancedMetaEngine<ADGameDescription> {
public:
    const char *getName() const override { return "xfiles"; }
    Common::Error createInstance(OSystem *syst, Engine **engine, const ADGameDescription *) const override {
        *engine=new XFiles::XFilesEngine(syst); return Common::kNoError;
    }
};
#if PLUGIN_ENABLED_DYNAMIC(XFILES)
REGISTER_PLUGIN_DYNAMIC(XFILES, PLUGIN_TYPE_ENGINE, XFilesMetaEngine);
#else
REGISTER_PLUGIN_STATIC(XFILES, PLUGIN_TYPE_ENGINE, XFilesMetaEngine);
#endif