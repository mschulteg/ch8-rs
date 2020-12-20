mod cpu;
mod perf;
mod loop_minifb_thread;
//mod loop_sdl2;

//use loop_minifb::event_loop;
use loop_minifb_thread::event_loop;


fn main() {
    event_loop();
}
