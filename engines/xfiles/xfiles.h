// SPDX-License-Identifier: GPL-3.0-or-later
#ifndef XFILES_ENGINE_H
#define XFILES_ENGINE_H
#include <algorithm>
#include "neodb.h"
#include "engines/engine.h"
#include "graphics/surface.h"
namespace XFiles {
struct MovieResource { uint32_t id; AssetEntry entry; };
class XFilesEngine : public Engine {
public:
	explicit XFilesEngine(OSystem *system) : Engine(system) {}
	Common::Error run() override;
private:
	NeoDb _title, _state;
	std::vector<MovieResource> _movies;
	Common::String _status;
	size_t _selected = 0;
	bool _abortIntro = false;
	bool load(const char *name, NeoDb &db, Common::String &failure);
	void collectMovies();
	void drawBrowser();
	void line(Graphics::Surface &surface, const Common::String &text, int y, uint32 color);
	bool play(const MovieResource &movie);
	void intro();
};
}
#endif
