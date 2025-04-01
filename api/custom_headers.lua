-- custom_headers.lua
-- wrk.method = "POST"
-- wrk.body = '{"app_id": "test1", "app_limit": 20}'
-- wrk.headers["Content-Type"] = "application/json"
-- Add your custom headers here
wrk.headers["X-GATEWAY-APPID"] = "test1"
-- wrk.headers["Authorization"] = "Bearer token123"

-- Optional: Print request info in setup
-- function setup(thread)
--    print("Thread setup: " .. thread:get("id"))
-- end

function request()
   return wrk.format()
end