pub unsafe fn exit_thread() -> ! {
    unsafe {
        winapi::um::processthreadsapi::ExitThread(0);
    }
}

pub use std::thread::{park, spawn, JoinHandle, Thread, ThreadId};

compile_error!(
    r"TODO: Sorry, y'all, this part ain't done yet!

                               QOQ        Q
                           Q$aa}wQ        NSSA
                        Bwo3jyyyw          P}jyhD
                      U3yyyyyy3ah          PjyayoZK
                   QwoyoyoyyyoyjG         $jztzl1FyGQ
                 QPyjyyyoooyyyyo3ZOQQ&pi;,,'''-''',~*h
                K3jyyyj3jyyo33oyyt?;,'.'--'''.'''''''';
              Najyyyjoyy33yya3x|^~:,,:~:,,''-'-',~^*izj
             pajyo3oyyjyoo3yz+~,''',,,'''',,:,'---''~rs
            Xyjoj3yjooyo3Zs>;~,,,'''''',,,,''',,,'-'.''~tQ          tL
           hjoyyyyjyyyoSj|~''-''-'''''--''',,,''',,,'----',r{%QQNa^,;
          Pojy3yyyoyja3x^,-'-'''''----''''---',:'''',~,'-.'--'''--,;Q
         Xy3jo3ySwwwS3l;'''---.---'--''--.---'--,:'-..',,_,,,',,,~*
        KSjyo}ywG|*ljGt,''--.--''-'-.',,-'--,,-''',:''---.''',,~*N
       QwyyyyyjUz*>*>*lZ\''-''-'''-''',;;,'',^;,---',,''''''~~\Q
       Uyojyyoj$)=*?>?>=\j;',~;;;;;;;~,';*;~,,+*+^;~,,~;;;^uN
      QSjyoy3yyXL**>**>*>>zL=?>******>>=^>**>;;^+>****>>>o
      hyyy3o3jjUt=*\*>*>*=>**>>****>*****+=*>>*>=>*****>>*3
      Gjyy3ooyyGj*>tl+>=>>>>*>*>>>>>+=>>*>>***>>*****>>>>>*Z
     QZyyy3yy3ya$L>>}\>>*>****>*>***>>>********>**>>*>>=**>>X
     gayyyyyyyyyX3+=>{z>>**>**>*>*>*>*>**>**>*>***>>>*+u?=*r|
     Dayyyyyyyyy3pt=>+\l?*>***==>*=+*>>***>**=*>^@t>*=>LgGlxQ@QQ
     RSyyyyyyyyyyZX1+*=***>>*+=|X}Xo=+++==>+>+r\@@=?>**>+|xs|+**X
     QwyyyyyyyyyyjjXy*+*>>*>=>;,=OQ@@@QQ&N&QQ@@@S^>>**>>\*?**>+zQ
      wyy3oyo33yo3yyaj}L>>>*=r^^;~>\)>+*|\z)|>+>***>=>**Xoi>>?}
      $yyooyyyyyyy3yy>~;*?***=~r+^+>**>*>>>=>>**=>**>>**oQQRQ
       Pyyyyyjyoyyy3S?,'';*?>>>>+>>*>**>**>>***>>>*>==>>sBQ
       QS3jjyyyyyy3ZS*,-',*>?>*>>*=?**>*****>>*********>L$R
        QP}yyyoojoy3S>,.-'~>***>>>>****>>>>>>>>>*****>==*lPN
          w3yoyojjooy+''-''~*>>*>>>>>>>>>>>>>>>>>=+=>>>*>+*j8
           %3jayyjoaj^,---''_+**>>>>>>>>>>>>>>>>>*>==+*LtwQ
             %SyyyySt;~'-'--',;>>>>>>>>>>>*>>>>>*Ll{G8
               Qho3wi~~,-.--..-'~=*>*>>+>*>>tQ
                    +'~_..-''-''''~+?*=>>>*>j
                   Q~',~.-''''''-'-';?*>>?>>Lg
"
);
