// SPDX-License-Identifier: GPL-3.0-or-later
#include "../engines/xfiles/script.h"
#include <iostream>
#include <cstdlib>
static void check(bool b,const char*n){if(!b){std::cerr<<"FAIL "<<n<<'\n';std::exit(1);}}
int main(){
	XFiles::Procedure p;
	std::vector<uint8_t> b(26,0);
	const std::string name="SyntheticProc";
	for(size_t i=0;i<name.size();++i)b[i]=name[i];
	b[15]=2;b[19]=7;b[23]=9;b[24]=2;b[25]=0;
	check(XFiles::parseProcedure(b,p) && p.name==name && p.args==std::vector<uint32_t>({7,9}),"procedure operands");
	for(size_t i=0;i<b.size();++i)check(!XFiles::parseProcedure(std::vector<uint8_t>(b.begin(),b.begin()+i),p),"truncated procedure");
	XFiles::Action a;
	check(XFiles::decodeAction(6,b,a) && a.kind()==6 && !a.conditional,"unconditional procedure");
	std::vector<uint8_t> prefix(12,0);prefix[3]=42;prefix[7]=1;prefix[8]=0;prefix[9]=2;prefix[10]=4;
	prefix.insert(prefix.end(),b.begin(),b.end());
	check(XFiles::decodeAction(0x86,prefix,a) && a.conditional && a.condition.lhs==42 && a.condition.op==4,"condition prefix");
	check(!XFiles::decodeAction(0x86,std::vector<uint8_t>(11,0),a),"truncated condition");
	check(XFiles::decodeAction(0,std::vector<uint8_t>(11,0),a),"eleven-byte variable body");
	check(!XFiles::decodeAction(4,std::vector<uint8_t>(15,0),a),"short set-view body");
	check(!XFiles::decodeAction(99,b,a),"unknown action is not silently executed");
	b[14]=255;b[15]=255;check(!XFiles::parseProcedure(b,p),"excessive argument count");
	XFiles::Operands o;
	check(!XFiles::parseOperands(b,size_t(-1),o),"operand offset overflow");
	std::cout<<"PASS: procedures, conditional actions, truncation, types and limits\n";
}
