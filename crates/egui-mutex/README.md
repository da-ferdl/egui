# egui_mutex

Simple lightweight mutex implementation for egui internal usage where a mutex is necessary.

**Attention: This mutex is lightweight because it is totally unfair and lacks all features found on the std and parking-lot mutex implementations!**
