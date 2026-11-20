// SPDX-License-Identifier: GPL-3.0-or-later
#ifndef XFILES_SHILOH_H
#define XFILES_SHILOH_H
#include "engines/engine.h"
namespace XFiles {
class ShilohEngine : public Engine {
public:
	explicit ShilohEngine(OSystem *system):Engine(system){}
	Common::Error run() override;
protected:
	void pauseEngineIntern(bool pause) override;
private:
	uint32 _lastTick=0;
};
}
#endif
