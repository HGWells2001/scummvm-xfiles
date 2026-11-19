// SPDX-License-Identifier: GPL-3.0-or-later
// VCAction layout reference: Shiloh world/action.rs (MIT), see THIRD_PARTY.md.
#ifndef XFILES_SCRIPT_H
#define XFILES_SCRIPT_H
#include "neodb.h"
namespace XFiles {
struct Operands {
	uint32_t lhs=0,rhs=0;
	uint8_t lhsKind=0,rhsKind=0,op=0;
};
struct Procedure {
	std::string name;
	std::vector<uint32_t> args;
	std::vector<uint8_t> kinds;
};
struct Action {
	uint32_t id=0;
	uint8_t type=0;
	bool conditional=false;
	Operands condition;
	std::vector<uint8_t> body;
	uint8_t kind() const {return type&0x7f;}
};
bool parseOperands(const std::vector<uint8_t> &data,size_t offset,Operands &out);
bool parseProcedure(const std::vector<uint8_t> &body,Procedure &out);
bool decodeAction(uint8_t type,const std::vector<uint8_t> &data,Action &out);
bool loadAction(const NeoDb &db,uint32_t id,Action &out);
}
#endif
