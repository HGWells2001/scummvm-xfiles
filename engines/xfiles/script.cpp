// SPDX-License-Identifier: GPL-3.0-or-later
#include "script.h"
namespace XFiles {
static uint32_t be32(const std::vector<uint8_t> &b,size_t p){
	return uint32_t(b[p])<<24|uint32_t(b[p+1])<<16|uint32_t(b[p+2])<<8|b[p+3];
}
bool parseOperands(const std::vector<uint8_t> &b,size_t p,Operands &out){
	out=Operands();
	if(p>b.size() || b.size()-p<11) return false;
	out.lhs=be32(b,p); out.rhs=be32(b,p+4);
	out.lhsKind=b[p+8];out.rhsKind=b[p+9];out.op=b[p+10];return true;
}
bool parseProcedure(const std::vector<uint8_t> &b,Procedure &out){
	out=Procedure();
	if(b.size()<16) return false;
	const size_t n=(size_t(b[14])<<8)|b[15];
	if(n>1024 || n>(b.size()-16)/5) return false;
	size_t end=0;while(end<14 && b[end])++end;
	if(!end) return false;
	Procedure p;
	p.name.assign(reinterpret_cast<const char *>(b.data()),end);
	for(size_t i=0;i<n;++i){p.args.push_back(be32(b,16+4*i));p.kinds.push_back(b[16+4*n+i]);}
	out=p;return true;
}
bool decodeAction(uint8_t type,const std::vector<uint8_t> &data,Action &out){
	out=Action();
	Action a;a.type=type;a.conditional=(type&0x80)!=0;
	size_t p=0;
	if(a.conditional){if(data.size()<12 || !parseOperands(data,0,a.condition))return false;p=12;}
	a.body.assign(data.begin()+p,data.end());
	static const size_t minimum[]={11,5,6,9,16,5,16,5};
	if(a.kind()>7 || a.body.size()<minimum[a.kind()]) return false;
	if(a.kind()==6){Procedure proc;if(!parseProcedure(a.body,proc))return false;}
	out=a;return true;
}
bool loadAction(const NeoDb &db,uint32_t id,Action &out){
	out=Action();
	std::vector<uint8_t> record,data;
	if(!db.recordBytes(0x41,id,15,record) || !db.blobAt(size_t(db.find(0x41,id))+6,data))return false;
	if(!decodeAction(record[14],data,out))return false;
	out.id=id;return true;
}
}
