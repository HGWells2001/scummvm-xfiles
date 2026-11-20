// SPDX-License-Identifier: GPL-3.0-or-later
// Optional in-process host. No external game executable or SDL window is started.
#include <vector>
#include "shiloh.h"
#ifdef USE_XFILES_SHILOH
#include "common/config-manager.h"
#include "common/error.h"
#include "common/events.h"
#include "common/system.h"
#include "common/textconsole.h"
#include "engines/util.h"
#include "graphics/cursorman.h"
#include "graphics/surface.h"
#include "audio/audiostream.h"
#include "audio/decoders/raw.h"
#include "audio/mixer.h"

extern "C" {
void *xfs_create(const char *,const char *,uint32,char *,size_t);
void xfs_destroy(void *);
int xfs_tick(void *,uint32);
int xfs_pointer(void *,uint32,int,int);
int xfs_key(void *,uint32,uint32);
int xfs_frame(void *,byte *,size_t);
int xfs_mix(void *,int16 *,size_t);
void xfs_error(const void *,char *,size_t);
uint32 xfs_view(const void *);
int xfs_cursor_shown(const void *);
}
namespace XFiles {
namespace {
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
	switch(c){
	case Common::KEYCODE_RETURN: case Common::KEYCODE_KP_ENTER:return 13;
	case Common::KEYCODE_ESCAPE:return 27;
	case Common::KEYCODE_BACKSPACE:return 8;
	case Common::KEYCODE_TAB:return 9;
	case Common::KEYCODE_SPACE:return 32;
	case Common::KEYCODE_DELETE:return 46;
	case Common::KEYCODE_LEFT:return 37;
	case Common::KEYCODE_UP:return 38;
	case Common::KEYCODE_RIGHT:return 39;
	case Common::KEYCODE_DOWN:return 40;
	case Common::KEYCODE_MINUS:return 0xbd;
	case Common::KEYCODE_PERIOD:return 0xbe;
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
Common::Error ShilohEngine::run(){
	const Common::String data=ConfMan.getPath("path").toString('/');
	const Common::String saves=ConfMan.getPath("savepath").join("xfiles-shiloh").toString('/');
	char message[1024]={0};
	const int boot=ConfMan.hasKey("boot_param")?ConfMan.getInt("boot_param"):0;
	void *core=xfs_create(data.c_str(),saves.c_str(),boot==-1?1:boot==-3?2:0,message,sizeof(message));
	if(!core)return Common::Error(Common::kReadingFailed,message);
	Runtime runtime(core,_mixer);
	initGraphics(640,480,nullptr);
	if(_system->getScreenFormat().bytesPerPixel<2)return Common::Error(Common::kUnsupportedColorMode);
	defaultCursor();
	std::vector<byte> rgb(640*480*3);
	const Graphics::PixelFormat source(3,8,8,8,0,0,8,16,0);
	Audio::QueuingAudioStream *audio=Audio::makeQueuingAudioStream(44100,true);
	_mixer->playStream(Audio::Mixer::kPlainSoundType,&runtime.sound,audio);runtime.playing=true;
	uint32 last=_system->getMillis();uint64 audioFraction=0;
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
				const uint32 key=virtualKey(event.kbd);
				if(key)result=xfs_key(core,key,((event.kbd.flags&Common::KBD_ALT)?1:0)|((event.kbd.flags&Common::KBD_SHIFT)?2:0));
				break;
			}
			default:break;
			}
			if(kind>=0)result=xfs_pointer(core,kind,event.mouse.x,event.mouse.y);
			if(result)break;
		}
		if(result)break;
		const uint32 now=_system->getMillis();
		if(now-last<16){_system->delayMillis(1);continue;}
		const uint32 dt=MIN<uint32>(now-last,250);last=now;
		result=xfs_tick(core,dt);if(result)break;
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
