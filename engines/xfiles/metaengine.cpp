// SPDX-License-Identifier: GPL-3.0-or-later
#include "xfiles.h"
#include "engines/advancedDetector.h"
#include "base/plugins.h"
#ifdef USE_XFILES_SHILOH
#include "shiloh.h"
#include "common/config-manager.h"
#endif
class XFilesMetaEngine : public AdvancedMetaEngine<ADGameDescription> {
public:
    const char *getName() const override { return "xfiles"; }
    int getAutosaveSlot() const override { return -1; }
    bool hasFeature(MetaEngineFeature f) const override {
#ifdef USE_XFILES_SHILOH
        return MetaEngine::hasFeature(f);
#else
        return false;
#endif
    }
    Common::Error createInstance(OSystem *syst, Engine **engine, const ADGameDescription *) const override {
#ifdef USE_XFILES_SHILOH
        if(!ConfMan.hasKey("xfiles_research") || !ConfMan.getBool("xfiles_research")){
            *engine=new XFiles::ShilohEngine(syst);return Common::kNoError;
        }
#endif
        *engine=new XFiles::XFilesEngine(syst); return Common::kNoError;
    }
};
#if PLUGIN_ENABLED_DYNAMIC(XFILES)
REGISTER_PLUGIN_DYNAMIC(XFILES, PLUGIN_TYPE_ENGINE, XFilesMetaEngine);
#else
REGISTER_PLUGIN_STATIC(XFILES, PLUGIN_TYPE_ENGINE, XFilesMetaEngine);
#endif
