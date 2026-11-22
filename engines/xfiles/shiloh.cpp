// SPDX-License-Identifier: GPL-3.0-or-later
// Optional in-process host. No external game executable or SDL window is started.
#include <vector>
#include "shiloh.h"
#ifdef USE_XFILES_SHILOH
#include "common/config-manager.h"
#include "common/error.h"
#include "common/events.h"
#include "common/savefile.h"
#include "common/system.h"
#include "common/textconsole.h"
#include "engines/util.h"
#include "engines/metaengine.h"
#include "graphics/cursorman.h"
#include "graphics/surface.h"
#include "audio/audiostream.h"
#include "audio/decoders/raw.h"
#include "audio/mixer.h"

#if defined(SDL_BACKEND) && defined(_WIN32)
extern "C" {
const char *SDL_GetHint(const char *);
int SDL_SetHintWithPriority(const char *,const char *,int);
int SDL_ResetHint(const char *);
}
#endif

extern "C" {
void *xfs_create(const char *,const char *,uint32,char *,size_t);
void xfs_destroy(void *);
int xfs_tick(void *,uint32);
int xfs_pointer(void *,uint32,int,int);
int xfs_key(void *,uint32,uint32);
int xfs_frame(void *,byte *,size_t);
int xfs_mix(void *,int16 *,size_t);
void xfs_error(const void *,char *,size_t);
int xfs_take_notice(void *, char *, size_t);
int xfs_can_save(const void *);
int xfs_export_save(void *, int (*)(void *, const byte *, size_t), void *);
int xfs_import_save(void *, const byte *, size_t);
uint32 xfs_view(const void *);
int xfs_cursor_shown(const void *);
}
namespace XFiles {
namespace {
constexpr uint32 kSaveMagic = MKTAG('X', 'F', 'S', '1');
constexpr uint32 kMaxSaveBytes = 128 * 1024 * 1024;

int collectSave(void *context, const byte *data, size_t length) {
	auto &payload = *static_cast<std::vector<byte> *>(context);
	payload.assign(data, data + length);
	return 1;
}
// Set before the engine's graphics transaction recreates the SDL renderer.
// Restore the previous hint when returning to the launcher.
struct SoftwareVideoHint {
#if defined(SDL_BACKEND) && defined(_WIN32)
	Common::String previous;bool hadPrevious;
	SoftwareVideoHint(){const char *p=SDL_GetHint("SDL_RENDER_DRIVER");hadPrevious=p!=nullptr;if(p)previous=p;SDL_SetHintWithPriority("SDL_RENDER_DRIVER","software",2);}
	~SoftwareVideoHint(){if(hadPrevious)SDL_SetHintWithPriority("SDL_RENDER_DRIVER",previous.c_str(),2);else SDL_ResetHint("SDL_RENDER_DRIVER");}
#endif
};
struct Runtime {
	void *core;
	Audio::Mixer *mixer;
	Audio::SoundHandle sound;
	bool playing=false;
	Runtime(void *c,Audio::Mixer *m):core(c),mixer(m){}
	~Runtime(){if(playing)mixer->stopHandle(sound);xfs_destroy(core);}
};
uint32 virtualKey(const Common::KeyState &key){
	const int c=key.keycode;
	if(c>=Common::KEYCODE_a && c<=Common::KEYCODE_z)return 'A'+c-Common::KEYCODE_a;
	if(c>=Common::KEYCODE_0 && c<=Common::KEYCODE_9)return '0'+c-Common::KEYCODE_0;
	if(c>=Common::KEYCODE_KP0 && c<=Common::KEYCODE_KP9)return '0'+c-Common::KEYCODE_KP0;
	switch(c){
	case Common::KEYCODE_F1:return 0x70;
	case Common::KEYCODE_F3:return 0x72;
	case Common::KEYCODE_RETURN: case Common::KEYCODE_KP_ENTER:return 13;
	case Common::KEYCODE_ESCAPE:return 27;
	case Common::KEYCODE_BACKSPACE:return 8;
	case Common::KEYCODE_TAB:return 9;
	case Common::KEYCODE_PAGEUP:return 0x21;
	case Common::KEYCODE_PAGEDOWN:return 0x22;
	case Common::KEYCODE_SPACE:return 32;
	case Common::KEYCODE_DELETE:return 46;
	case Common::KEYCODE_LEFT:return 37;
	case Common::KEYCODE_UP:return 38;
	case Common::KEYCODE_RIGHT:return 39;
	case Common::KEYCODE_DOWN:return 40;
	case Common::KEYCODE_MINUS: case Common::KEYCODE_KP_MINUS:return 0xbd;
	case Common::KEYCODE_PERIOD: case Common::KEYCODE_KP_PERIOD:return 0xbe;
	case Common::KEYCODE_COMMA:return 0xbc;
	default:return 0;
	}
}
void defaultCursor(){
	byte pixels[12*16];memset(pixels,255,sizeof(pixels));
	for(int y=0;y<12;++y)for(int x=0;x<=y/2;++x)pixels[y*12+x]=(x==0 || x==y/2 || y==11)?1:0;
	for(int y=10;y<16;++y){pixels[y*12+3]=1;pixels[y*12+4]=0;pixels[y*12+5]=1;}
	const byte palette[]={255,255,255,0,0,0};
	CursorMan.replaceCursor(pixels,12,16,0,0,255);
	CursorMan.replaceCursorPalette(palette,0,2);CursorMan.showMouse(true);
}
}
bool ShilohEngine::hasFeature(EngineFeature f) const {
	return f == kSupportsSavingDuringRuntime || f == kSupportsLoadingDuringRuntime || f == kSupportsReturnToLauncher;
}

bool ShilohEngine::canSaveGameStateCurrently(Common::U32String *msg) {
	if (xfs_can_save(_core))
		return true;
	if (msg)
		*msg = Common::U32String("Attendi che la scena sia pronta per il salvataggio.");
	return false;
}

bool ShilohEngine::canLoadGameStateCurrently(Common::U32String *) {
	return _core != nullptr;
}

Common::Error ShilohEngine::saveGameState(int slot, const Common::String &desc, bool isAutosave) {
	if (slot < 0 || slot > getMetaEngine()->getMaximumSaveSlot())
		return Common::kWritingFailed;
	std::vector<byte> payload;
	if (xfs_export_save(_core, collectSave, &payload) != 0) {
		char error[512] = {};
		xfs_error(_core, error, sizeof(error));
		return Common::Error(Common::kWritingFailed, error);
	}
	Common::ScopedPtr<Common::OutSaveFile> file(_saveFileMan->openForSaving(getSaveStateName(slot)));
	if (!file)
		return Common::kWritingFailed;
	file->writeUint32BE(kSaveMagic);
	file->writeUint32LE(payload.size());
	if (file->write(payload.data(), payload.size()) != payload.size() || file->err())
		return Common::kWritingFailed;
	getMetaEngine()->appendExtendedSave(file.get(), getTotalPlayTime(), desc, isAutosave);
	return file->err() ? Common::kWritingFailed : Common::kNoError;
}

Common::Error ShilohEngine::loadGameStream(Common::SeekableReadStream *stream) {
	if (stream->readUint32BE() != kSaveMagic)
		return Common::kReadingFailed;
	const uint32 size = stream->readUint32LE();
	if (size < 24 || size > kMaxSaveBytes || size > stream->size() - stream->pos())
		return Common::kReadingFailed;
	std::vector<byte> payload(size);
	if (stream->read(payload.data(), size) != size || stream->err())
		return Common::kReadingFailed;
	if (xfs_import_save(_core, payload.data(), size) != 0) {
		char error[512] = {};
		xfs_error(_core, error, sizeof(error));
		return Common::Error(Common::kReadingFailed, error);
	}
	return Common::kNoError;
}

void ShilohEngine::pauseEngineIntern(bool pause){
	Engine::pauseEngineIntern(pause);
	// A modal ScummVM pause must not advance the game's timers on resume.
	_lastTick=_system->getMillis();
}
Common::Error ShilohEngine::run(){
	const Common::String data=ConfMan.getPath("path").toString('/');
	Common::Path saveBase=ConfMan.getPath("savepath");
	if(saveBase.empty())saveBase=_system->getDefaultConfigFileName().getParent().join("saves");
	const Common::String saves=saveBase.join("xfiles-shiloh").toString('/');
	char message[1024]={0};
	const int boot=ConfMan.hasKey("boot_param")?ConfMan.getInt("boot_param"):0;
	void *core=xfs_create(data.c_str(),saves.c_str(),boot==-1?1:boot==-3?2:0,message,sizeof(message));
	if(!core)return Common::Error(Common::kReadingFailed,message);
	Runtime runtime(core,_mixer);
	_core = core;
	struct ResetCore {
		void *&core;
		~ResetCore() { core = nullptr; }
	} resetCore{_core};
	SoftwareVideoHint videoHint;
	initGraphics(640,480,nullptr);
	if(_system->getScreenFormat().bytesPerPixel<2)return Common::Error(Common::kUnsupportedColorMode);
	defaultCursor();
	if (ConfMan.hasKey("save_slot")) {
		const int slot = ConfMan.getInt("save_slot");
		if (slot >= 0) {
			const Common::Error result = loadGameState(slot);
			if (result.getCode() != Common::kNoError)
				return result;
		}
	}
	std::vector<byte> rgb(640*480*3);
	const Graphics::PixelFormat source(3,8,8,8,0,0,8,16,0);
	Audio::QueuingAudioStream *audio=Audio::makeQueuingAudioStream(44100,true);
	_mixer->playStream(Audio::Mixer::kPlainSoundType,&runtime.sound,audio);runtime.playing=true;
	_lastTick=_system->getMillis();uint64 audioFraction=0;
	int result=0;
	while(!shouldQuit()){
		Common::Event event;
		while(_eventMan->pollEvent(event)){
			int kind=-1;
			switch(event.type){
			case Common::EVENT_MOUSEMOVE:kind=0;break;
			case Common::EVENT_LBUTTONDOWN:kind=1;break;
			case Common::EVENT_LBUTTONUP:kind=2;break;
			case Common::EVENT_RBUTTONDOWN:kind=3;break;
			case Common::EVENT_RBUTTONUP:kind=4;break;
			case Common::EVENT_WHEELUP:kind=5;break;
			case Common::EVENT_WHEELDOWN:kind=6;break;
			case Common::EVENT_KEYDOWN:{
				if (event.kbd.keycode == Common::KEYCODE_F5 && !(event.kbd.flags & Common::KBD_NON_STICKY)) {
					if (!event.kbdRepeat)
						openMainMenuDialog();
					break;
				}
				if (event.kbdRepeat && (event.kbd.keycode == Common::KEYCODE_F1 ||
				                        event.kbd.keycode == Common::KEYCODE_F3))
					break;
				const uint32 key=virtualKey(event.kbd);
				if(key)result=xfs_key(core,key,((event.kbd.flags&Common::KBD_ALT)?1:0)|((event.kbd.flags&Common::KBD_SHIFT)?2:0)|(event.kbdRepeat?4:0));
				break;
			}
			default:break;
			}
			if(kind>=0)result=xfs_pointer(core,kind,event.mouse.x,event.mouse.y);
			if(result)break;
		}
		if(result)break;
		const uint32 now=_system->getMillis();
		if(now-_lastTick<16){_system->delayMillis(1);continue;}
		const uint32 dt=MIN<uint32>(now-_lastTick,250);_lastTick=now;
		result=xfs_tick(core,dt);if(result)break;
		if (xfs_take_notice(core, message, sizeof(message)))
			_system->displayMessageOnOSD(Common::U32String(message));
		result=xfs_frame(core,rgb.data(),rgb.size());if(result)break;
		Graphics::Surface raw;raw.init(640,480,640*3,rgb.data(),source);
		Graphics::Surface *converted=raw.convertTo(_system->getScreenFormat());
		if(!converted)return Common::Error(Common::kUnknownError,"Framebuffer conversion failed");
		_system->copyRectToScreen(converted->getPixels(),converted->pitch,0,0,640,480);
		converted->free();delete converted;
		CursorMan.showMouse(!xfs_cursor_shown(core));_system->updateScreen();
		if(boot==-1 || boot==-3){
			warning("XFILES Shiloh smoke: rendered 640x480, view=%u",xfs_view(core));
			return Common::kNoError;
		}
		if(audio->numQueuedStreams()<6){
			audioFraction+=uint64(dt)*44100;const size_t frames=audioFraction/1000;audioFraction%=1000;
			if(frames){
				int16 *pcm=(int16 *)malloc(frames*2*sizeof(int16));
				if(!pcm)return Common::Error(Common::kUnknownError,"Audio allocation failed");
				result=xfs_mix(core,pcm,frames*2);
				if(result){free(pcm);break;}
				audio->queueBuffer((byte *)pcm,frames*2*sizeof(int16),DisposeAfterUse::YES,Audio::FLAG_16BITS|Audio::FLAG_STEREO|Audio::FLAG_LITTLE_ENDIAN);
			}
		}
	}
	if(result<0){xfs_error(core,message,sizeof(message));return Common::Error(Common::kUnknownError,message);}
	return Common::kNoError;
}
}
#endif
