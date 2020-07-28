local a = "hello"
local b = "world"
local c = {}
c[1] = a
c[2] = b
c[3] = 0.4
c[4] = a .. b

function d(c)
	return c
end

d(c)

return d;
