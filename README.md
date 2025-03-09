
# Introduction

Assigments: [C/Lua task](https://docs.google.com/document/d/1tNfj7IRdg51rtea-4_QhueqDQEAXjAelJLgbElUfcP0/edit?tab=t.0),  [GO / DNS](https://docs.google.com/document/d/1_bJg-zJyoDaY-xHnl1ZBxXwmQ30SwTvngdpMVofDG-E/edit?tab=t.0#heading=h.lg5ipse8t11a)

For the recruitment task, I selected the combo of C/Lua task and DNS task. My primary motivation was that I was aware of the importance of NGINX in CDN77's stack. As I didn't have much experience with it before, I wanted to give it a proper look, understand it on a deeper level and get some hands-on experience. I assume that this knowledge would probably always be valuable in the real day-to-day job at CDN. Another reason was that all of the tasks seemed pretty straightforward, and I could easily imagine how I would proceed in each one of them.

In the next part, I will just present the solution in the most direct way. Afterwards in the log, I will try to describe how I worked on each part in further detail, and shed a light on my thinking process.

# Solutions

## NGINX

### Caching explained
The cache key is an unique identifier that NGINX assigns to each cacheable resource.

It is built up by appending several variables, by default from the `scheme`, `Host` header, `uri`, and any string arguments. Variables can be changed with the `proxy_cache_key` directive. After the variables are combined, final key is created by using the *MD5 hash* sum.

Function `ngx_http_file_cache_create_key` for creating the key is located in `ngx_http_file_cache.c`. The key is being saved in the member values of `ngx_http_cache_t` as `u_char*`. 

NGINX caches each resource by the cache key. All requests that generate the same cache key will be satisfied with the same resource. So the key is used as an identificator for saving and for searching resources in the cache.

The minimized version of `nginx.conf` I used to turn on caching:
```nginx
http {
	server {
		location / {
			proxy_pass http://origin_server; 
			proxy_cache my_cache;
			proxy_cache_valid any 10m;
			add_header X-Proxy-Cache $upstream_cache_status;
		}
	}
	proxy_cache_path /var/cache/nginx levels=1:2 keys_zone=my_cache:10m max_size=1g inactive=1d use_temp_path=off;
}
```

The `proxy_cache_path` directive defines various settings for storing cached content. Cached data is saved on disk in a specified location, following a hierarchical directory structure. The `levels=1:2` parameter indicates a two-level cache, meaning a two-tiered directory hierarchy. The cached resources are stored as individual files, named according to their cache key. The hierarchical directories organize these files using a red-black tree structure, where directory names correspond to suffixes of the cache key, with lengths matching the directory depth. Each cached file contains headers, a small amount of metadata, and the cached value itself.

Here is an example of cached resources path on the system: `/var/cache/nginx/7/4f/3b2a3d5e8c6a4891c5d1f982ba742 `

### X-Cache-Key implementation

All of the changes to enable the X-Cache-Key header are in [this commit](https://github.com/lmnek/nginx/commit/6359e114aa8a200d006a7b0e5595e2b8455f771c) 🚨.

To try it out, just pull the repo, compile it and run. No further configuration should be necessary. The header will be automatically added to all responses.

NGINX response compared with the cache directory:
![[cache-key-demo.png]]

### Wildcard algorithm

I will describe DNS wildcard algorithm that lives inside of `ngx_http_referer_module`. 

The algorithm revolves around the `ngx_hash_combined_t` data structure, which is essentially an enhanced hash table composed of three distinct sub-tables: one for exact (non-wildcard) values, one for head wildcards (e.g., `*.example.com`), and one for tail wildcards (e.g., `example.*`).

Validation of the referer header occurs in the `ngx_http_referer_variable` function. Initially, the algorithm verifies whether the required data structures have been initialized. It then checks the presence and validity of the referer header, extracting it if necessary. Next, the URL scheme (`http://` or `https://`) is stripped from the referer, leaving only the domain to be validated. This domain's validity is compared against the schemes specified by the referer directive, which happens by computing hash of the value and invoking `ngx_hash_find_combined` that searches the wildcard hash table data structure.

This function sequentially searches through the three hash tables:
1. **Exact match table:** It first attempts a direct match using the hash value to locate the appropriate bucket. If the domain matches exactly within this bucket, it immediately returns a positive result.
2. **Head wildcard table:** If no exact match is found, the algorithm calls `ngx_hash_find_wc_head` to determine if the domain matches any head wildcard rule (`*.domain.com`). This function locates the last dot in the domain name, extracts the substring after it, computes its hash, and performs a lookup. If a match occurs, special flag bits associated with the entry indicate whether this match applies directly or if a further recursive search within another wildcard table is required.
3. **Tail wildcard table:** If neither of the previous searches yield a result, the algorithm invokes `ngx_hash_find_wc_tail` to assess tail wildcard matches (`domain.*`). Here, it locates the first dot from the left, extracts the preceding substring, computes its hash, and conducts a lookup. Similarly, if a match is found, the stored pointer may directly confirm the match or point to another wildcard hash table, prompting another recursive lookup.

If any of these three lookups result in a match, the referer is validated and considered legitimate. If all lookups fail, the referer is deemed invalid and rejected.

This approach allows efficient validation, which effectively handles both exact and wildcard referer rules without exhaustive scans.

### Lua interop

The C function:
```c
int fibonacci_iter(int a, int b) { return a + b; }
```

Compilling into .so library:
```bash
gcc -shared -o fib_lib.so -fPIC fib_lib.c
```

`nginx.conf` endpoint that calls the C function in Lua and returns the result:
```nginx
location /lua {
	content_by_lua_block {
		local ffi = require("ffi")

		ffi.cdef[[
			int fibonacci_iter(int a, int b);
		]]
		local c_lib = ffi.load("/opt/nginx/lib/fib_lib.so")

		local result = c_lib.fibonacci_iter(5, 8)
		ngx.say("Result: " .. result)
	}
}
```

## DNS

### Data structure

Right after reading the assigment, I had many ideas on how the problem could be approached. It came to me naturally that it could be formulated as an instance of a **prefix search** problem, which usually utilizes the prefix tree data structure (*Trie*). An algorithm using Trie would immidiately have lookup asymptotic complexity better than linear, but in its pure form it would be far away from being space optimal.

So I conducted a research on various modifications of Tries, and found many versions which reduce the space necessary to store them. The most promissing ones were *Radix trees*, or Compression tries or their special variant *Patricia trie*. The main idea for optimizing space is called **path-compression**, which essentialy eliminates branches of nodes with a single child. The space optimalizations also indirectly positively impact the performance of lookup, because the algorithm has to step through less nodes than in the uncrompressed variant of the tree.

The next step is to compress further by incorporating the so called **level-compression**, which results in reduced depth of the tree and more balance. **LC-Trie** compresses both paths and levels, and is the data structure I chose to go with. According to my research it is commonly used for IP lookups (similar task), for example even in the Linux kernel. The space complexity is definitely close to optimum, and practically the memory footprint can be even reduced by using a prefix table for lookup of nodes instead of storing references to children. The performance benchmarks from multiple source also suggest that it is one of the most efficient structures for lookups. 

The disavantage of LC-Tries are that the insert/delete operations are much more computationally difficult than lookups, so they are possibly not suitable if the routing data change very often. On the other hand, the lookups should be very quick even for millions of IPs, and it handles scaling great in that regard. 

Disclaimer: LC-Tries are not necessarily more space efficient than normal Radix trees. The main difference is in the lookup speed.

### Code

The prototype implementations are in the files [main.rs](https://github.com/lmnek/cdn77_task/blob/main/dns_task/src/main.rs) and [lc_trie.rs](https://github.com/lmnek/cdn77_task/blob/main/dns_task/src/lc_trie.rs).

# Log

I generally work in a way where I spend considerable amount of time researching about the task's subject, making my own notes, and trying to understand the problem from different angles. Afterwards I start implementing, while already having ideas and potential solutions figured out. But this process still serves as a brainstorming/experimentation session, because I usually encounter new issues and need to further refine my implementation. Eventually when I converge to a solution I'm happy with, I then enjoy making it look more "pretty", idiomatical and readable. I'm describing it as I applied the same process to most of the parts of this recruitment task.

## nginx

Researching for the first part definitely took me the most time, as I used NGINX only once in my life and very briefly. I've spent around 3 days getting familiar with nginx in various ways - reading docs, blogs, understanding the architecture, looking at different usecases, config structure and directives, and especially exploring the codebase.

I then jumped onto compiling nginx from source and running it to see if everything would work correctly on my system. I followed the instructions step by step and everything seemed very clear. The only problem I stumbled upon was permission issues with some directories, which I fixed after playing with the users/groups on my system. If I installed NGINX again next time, I would probably try to run inside of Docker, so I don't pollute my system as much. 

It was pretty easy to identify necessary function in the code and understand how the cache key is computed. I just skimmed through the codebase, looked at documentation for different modules, and searched for some key words with Grep. After realizing how it works, just to be sure I didn't make any error, I double checked by computing the md5 hash manually and compared it with file name in the cache directory.

The implementation of *X-Cache-Key* header was in the end also simple and straight-forward, although I had some difficulties figuring out what are the correct functions and data types to use in the context of the codebase. In many iterations I also managed to write a code that crashed the server, for example because of issues with handling pointers and sizes of arrays (also of-by-one errors 💀). Most of the day I spent searching for these issues and selecting appropriate data types/functions to put in my code. This led me to gain better intution on nginx internals, and I had the opportunity to play with logs and also do basic debugging. After I got the functionality working, I extracted the code into a separate function, cleaned it up a bit, and did some minor optimalizations (e.g. replaced redundant static local array with smarter handling of dynamically allocated array). I tried to follow the coding style and conventions of the codebase to the best of my abilities.

I fullfiled the goal of the assigment, ie. returning the cache key as a header in every request. Of course it isn't desirable to return the header with absolutely every request.  I chose to append the key to headers right after the key is created, but in production it could be worth it to have it as an option under a specific directive, or to move the function into the module with header filters.

There shouldn't be any real bottleneck in the code. Only the conversion of key to string (with hexdump function) is possibly redundant, because the same operation might happen again later in a different place.

The implementation together with the setup of nginx took me around a whole day of work, mostly because I got stuck so many times on the "core dumped" issues :D.

In contrast the following tasks were smooth sailing. For the DNS wildcard matching, I tried to reverse engineer functions which included the relevant algorithm. Reading up on the used data types in the dev documentation helped a lot, and enabling logging cleared up any misunderstandings. For the Lua integration, I just followed the docs and finished without encoutering any problems. Both of these tasks together took me around 5 hours.

## DNS

I've described the decision process of LC-Trie pretty thoroughly in the solution, so I will not go through the thinking process again. Just for the sake of completeness, during my research I've also stumbled on other advanced techniques and data structures, that I did not choose to go with and that could also result in performant and scalable solutions, for example HAT Tries, structures utilizing parallel/distributed computing, adaptive radix trees and multiple Sufix tries.

Because I don't have any experience with Go, I chose to implement my solution in Rust. It can achieve the same ⚡blazingly fast⚡ performance as Go, and I find it convenient to quickly sketch out my ideas in it. Rust compiler and type system also help in uncovering some obvious bugs, that could go unnoticed in other languages. 

The goal of the task was to implement the only `route` method, so I chose to create a prototype that has only the necessary functions. The main point of the implementation is to illustrate the idea, which could be potentially expanded into functional solution in the future.

I created a struct for LC-Trie in `lc_trie.rs` and implemented the related `lookup` function. All of the other functions for the data structure, like `insert` and `delete`, were ommited because it would take significantly more time to implement them, as they would need to include the logic for compressions and re-balacing of the tree. For the implementation I closely followed the [original paper for LC-Tries](https://www.drdobbs.com/cpp/fast-ip-routing-with-lc-tries/184410638) and adapted it for Rust needs.

The `main.rs` includes the `route` method that operates on the ecs and data store in LC-Trie, and `main` function that illustrates aproximately how the routing function could be used. 

Of course the prototype is very incomplete - when working in real environment, I would first finish all of the necessary methods for LC-Trie. It could be also worth it to refactor the LC-Trie struct and functions to be generic, so they could be used in other contexts with different datatypes. Afterwards I would create a set of unit tests, both for the Trie and for the `route` function in respect to the RFC. Then I would try to conduct profiling on the method and further optimize the code for performance. 

Altogether, I've spent half-a-day researching and brainstorming, and the other implementing and cleaning up the code.

# Conclusion

Overall, I've successfully completed all the given tasks. While I know there's always room for improvement, I gave my best efforts within the limited timeframe, and I hope my work clearly demonstrates my approach and skills.

If this assignment was also meant as a way to show whether I would find the role enjoyable and worthwhile, it certainly confirmed that I would. Working with such an extensive C codebase and exploring the utilities and data structures surrounding it was genuinely interesting. The interoperability between Lua and C was particularly impressive, because I didn't anticipate it working so seamlessly. Additionally, researching advanced data structures for the DNS task was also big fun, as it introduced me to concepts I previously had little knowledge about.

The primary challenge I encountered was underestimating the time required - I ended up taking over a week in total. This was partly due to my tendency toward perfectionism. I find it difficult to submit work that doesn't meet my own standards of quality. Each task also led me into deep research, resulting in extensive notes on related topics.

I genuinely hope my completed tasks reflect positively on my working style and align with the culture at CND77. I'm very much looking forward to the opportunity of discussing this further at an in-person interview! 🫡
