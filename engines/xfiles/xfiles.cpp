// SPDX-License-Identifier: GPL-3.0-or-later
#include "xfiles.h"
#include "script.h"
#include "common/archive.h"
#include "common/config-manager.h"
#include "common/events.h"
#include "common/file.h"
#include "common/fs.h"
#include "common/system.h"
#include "common/textconsole.h"
#include "engines/util.h"
#include "graphics/font.h"
#include "graphics/fontman.h"
#include "video/qt_decoder.h"

namespace XFiles {
bool XFilesEngine::load(const char *name, NeoDb &db, Common::String &failure) {
	Common::File f;
	if (!f.open(name)) { failure=Common::String::format("File mancante: %s",name); return false; }
	if (f.size()<24 || f.size()>128*1024*1024) { failure="Dimensione database non valida"; return false; }
	std::vector<uint8_t> bytes((size_t)f.size());
	if (f.read(bytes.data(),(uint32)bytes.size())!=bytes.size()) { failure="Database troncato"; return false; }
	if (!db.load(bytes.data(),bytes.size())) { failure=db.error().c_str(); return false; }
	return true;
}
void XFilesEngine::collectMovies() {
	_movies.clear();
	for (const NeoClass &cls:_title.classes()) {
		if (cls.id!=0x35) continue;
		for (const auto &obj:cls.objects) {
			AssetDescriptor info;
			if (!_title.assetInfo(obj.first,info)) continue;
			for(const AssetEntry &entry:info.entries) {
				Common::String name(entry.file.c_str()); name.toLowercase();
				if (!(name.hasSuffix(".xmv") || name.hasSuffix(".nmv") || name.hasSuffix(".amv") || name.hasSuffix(".dmv"))) continue;
				Common::File f;
				if (!f.open(Common::Path(entry.file.c_str(),'/')) || f.size()<8) continue;
				f.readUint32BE(); const uint32 type=f.readUint32BE();
				if(type!=MKTAG('m','o','o','v') && type!=MKTAG('m','d','a','t')) continue;
				_movies.push_back(MovieResource{obj.first,entry});
			}
		}
	}
}
void XFilesEngine::line(Graphics::Surface &surface,const Common::String &text,int y,uint32 color) {
	FontMan.getFontByUsage(Graphics::FontManager::kConsoleFont)->drawString(&surface,text,16,y,608,color);
}
void XFilesEngine::drawBrowser() {
	Graphics::Surface screen;
	screen.create(640,480,_system->getScreenFormat());
	screen.fillRect(Common::Rect(640,480),screen.format.RGBToColor(7,15,18));
	const uint32 white=screen.format.RGBToColor(225,235,232), green=screen.format.RGBToColor(96,210,160);
	line(screen,"THE X-FILES (1998) - ScummVM sperimentale",16,green);
	line(screen,"Ricerca risorse e video. Gioco completo NON ancora giocabile.",36,white);
	line(screen,Common::String::format("Database: %u classi + %u stato | %u riferimenti video locali",
		(unsigned)_title.classes().size(),(unsigned)_state.classes().size(),(unsigned)_movies.size()),60,white);
	line(screen,"Frecce: seleziona | Invio: riproduci | I: introduzione",86,green);
	line(screen,"PagSu/PagGiu: pagina | Esc: esci | Video: Spazio pausa, Esc torna",104,green);
	const size_t first=(_selected/16)*16;
	for(size_t i=first;i<_movies.size() && i<first+16;++i) {
		const MovieResource &m=_movies[i];
		line(screen,Common::String::format("%c %5u v%u %-24s %u..%u ms",
			i==_selected?'>':' ',m.id,m.entry.variant,m.entry.file.c_str(),m.entry.start,m.entry.end),
			138+int(i-first)*17,i==_selected?green:white);
	}
	line(screen,Common::String::format("Risorsa %u di %u",_movies.empty()?0:(unsigned)_selected+1,(unsigned)_movies.size()),426,green);
	line(screen,_status,450,white);
	_system->copyRectToScreen(screen.getPixels(),screen.pitch,0,0,640,480);
	_system->updateScreen(); screen.free();
}
bool XFilesEngine::play(const MovieResource &movie) {
	Video::QuickTimeDecoder decoder;
	if (!decoder.loadFile(Common::Path(movie.entry.file.c_str(),'/'))) {
		_status=Common::String::format("Formato non ancora decodificato: %s",movie.entry.file.c_str()); return false;
	}
	if (!decoder.getWidth() || !decoder.getHeight() || decoder.getWidth()>640 || decoder.getHeight()>480) {
		_status="Dimensioni video non supportate dal visualizzatore"; return false;
	}
	if (movie.entry.start && !decoder.seek(Audio::Timestamp(0,movie.entry.start,1000))) {
		_status="Il decoder non raggiunge il segmento richiesto"; return false;
	}
	_system->fillScreen(0); _system->updateScreen(); decoder.start();
	bool done=false;
	while (!shouldQuit() && !decoder.endOfVideo() && !done) {
		Common::Event e;
		while (_eventMan->pollEvent(e)) {
			if (e.type==Common::EVENT_KEYDOWN && e.kbd.keycode==Common::KEYCODE_SPACE) decoder.pauseVideo(!decoder.isPaused());
			if (e.type==Common::EVENT_KEYDOWN && e.kbd.keycode==Common::KEYCODE_ESCAPE) { done=true; _abortIntro=true; }
			if (e.type==Common::EVENT_LBUTTONDOWN || e.type==Common::EVENT_RBUTTONDOWN) done=true;
		}
		if (movie.entry.end && decoder.getTime()>=movie.entry.end) break;
		if (!done && decoder.needsUpdate()) {
			const Graphics::Surface *frame=decoder.decodeNextFrame();
			if (frame) {
				Graphics::Surface *converted=frame->convertTo(_system->getScreenFormat(),decoder.getPalette());
				if (!converted) { _status="Conversione colore fallita"; return false; }
				_system->copyRectToScreen(converted->getPixels(),converted->pitch,(640-converted->w)/2,(480-converted->h)/2,converted->w,converted->h);
				_system->updateScreen(); converted->free(); delete converted;
			}
		}
		_system->delayMillis(10);
	}
	_status=Common::String::format("Video: %s",movie.entry.file.c_str()); return true;
}
void XFilesEngine::intro() {
	// Cold-start order from Shiloh front/intro.rs; assets resolved through this edition's DB.
	const uint32 ids[]={0xdac3,0xdac2,0x4cd4,0xdac1};
	_abortIntro=false;
	for(uint32 id:ids) {
		if (shouldQuit() || _abortIntro) break;
		bool found=false;
		for(const MovieResource &m:_movies) if(m.id==id){ found=true; play(m); break; }
		if(!found) _status=Common::String::format("Introduzione: risorsa %u non disponibile",id);
	}
}
Common::Error XFilesEngine::run() {
	Common::FSNode root(ConfMan.getPath("path"));
	SearchMan.addDirectory("xfiles-root",root,0,3);
	if(root.getName().equalsIgnoreCase("MININST") || root.getName().equalsIgnoreCase("MEDINST"))
		SearchMan.addDirectory("xfiles-media",root.getParent(),1,3);
	SearchMan.addDirectory("xfiles-mininst",root.getChild("MININST"),0,2);
	SearchMan.addDirectory("xfiles-medinst",root.getChild("MEDINST"),0,2);
	Common::String failure;
	if(!load("XFILES.HDB",_title,failure) || !load("XFILES.GAM",_state,failure)) return Common::Error(Common::kReadingFailed,failure);
	initGraphics(640,480,nullptr);
	if(_system->getScreenFormat().bytesPerPixel==1) return Common::Error(Common::kUnsupportedColorMode);
	collectMovies(); _status="Anteprima sperimentale: nessuna modifica ai dati originali."; drawBrowser();
	if(ConfMan.hasKey("boot_param") && ConfMan.getInt("boot_param")==-1){
		warning("XFILES smoke: title=%u state=%u movies=%u",(unsigned)_title.classes().size(),(unsigned)_state.classes().size(),(unsigned)_movies.size());
		return _movies.empty()?Common::Error(Common::kReadingFailed,"Nessun filmato QuickTime locale"):Common::Error(Common::kNoError);
	}
	while(!shouldQuit()) {
		Common::Event e; bool dirty=false;
		while(_eventMan->pollEvent(e)) {
			if(e.type!=Common::EVENT_KEYDOWN) continue;
			dirty=true;
			switch(e.kbd.keycode) {
			case Common::KEYCODE_ESCAPE: return Common::kNoError;
			case Common::KEYCODE_UP: if(_selected) --_selected; break;
			case Common::KEYCODE_DOWN: if(_selected+1<_movies.size()) ++_selected; break;
			case Common::KEYCODE_PAGEUP: _selected=_selected>16?_selected-16:0; break;
			case Common::KEYCODE_PAGEDOWN: if(!_movies.empty()) _selected=std::min(_selected+16,_movies.size()-1); break;
			case Common::KEYCODE_RETURN: if(!_movies.empty()) play(_movies[_selected]); break;
			case Common::KEYCODE_i: intro(); break;
			case Common::KEYCODE_d: {
				unsigned decoded=0,unsupported=0;
				for(const NeoClass &cls:_title.classes())if(cls.id==0x41)for(const auto &entry:cls.objects){Action a;if(loadAction(_title,entry.first,a))++decoded;else ++unsupported;}
				_status=Common::String::format("Azioni decodificate: %u | sconosciute: %u (esecuzione non implementata)",decoded,unsupported);
				break;
			}
			default: break;
			}
		}
		if(dirty && !shouldQuit()) drawBrowser();
		_system->delayMillis(10);
	}
	return Common::kNoError;
}
}
