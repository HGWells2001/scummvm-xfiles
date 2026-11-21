// SPDX-License-Identifier: GPL-3.0-or-later
#ifndef XFILES_SHILOH_H
#define XFILES_SHILOH_H
#include "engines/engine.h"
namespace XFiles {
class ShilohEngine : public Engine {
public:
	explicit ShilohEngine(OSystem *system):Engine(system){}
	Common::Error run() override;
	bool hasFeature(EngineFeature f) const override;
	bool canSaveGameStateCurrently(Common::U32String *msg = nullptr) override;
	bool canLoadGameStateCurrently(Common::U32String *msg = nullptr) override;
	Common::Error saveGameState(int slot, const Common::String &desc, bool isAutosave = false) override;
	Common::Error loadGameStream(Common::SeekableReadStream *stream) override;
protected:
	int getAutosaveSlot() const override { return -1; }
	void pauseEngineIntern(bool pause) override;
private:
	void *_core = nullptr;
	uint32 _lastTick=0;
};
}
#endif
