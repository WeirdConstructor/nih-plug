if is_some[BG_THREAD] {
    BG_THREAD_QUIT.write $t;
    BG_THREAD.join[];
    log "Thread Quit";
};

BG_THREAD_QUIT.write $f;
.BG_THREAD = std:thread:spawn $code {
    !@wlambda;
    !@import std;
    while not[BG_THREAD_QUIT.read[]] {
        log_sender.send "TICK 2";
        std:thread:sleep :ms => 250;
    };
    log_sender.send "QUIT!";
} ${ BG_THREAD_QUIT = BG_THREAD_QUIT, log_sender = new_log_sender[] };

{!(note, on) = @;
    log ~ $F"Note: {} = {}" note on;
}
