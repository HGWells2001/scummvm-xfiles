MODULE := engines/xfiles
MODULE_OBJS := metaengine.o neodb.o asset.o script.o xfiles.o shiloh.o
ifeq ($(ENABLE_XFILES), DYNAMIC_PLUGIN)
PLUGIN := 1
endif
include $(srcdir)/rules.mk
DETECT_OBJS += $(MODULE)/detection.o
