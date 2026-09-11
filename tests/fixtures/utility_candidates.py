# Small utility functions with shared patterns.

def clamp(value, min_val, max_val):
    if value < min_val:
        return min_val
    if value > max_val:
        return max_val
    return value


def constrain(x, lower, upper):
    if x < lower:
        return lower
    if x > upper:
        return upper
    return x


def limit_range(n, lo, hi):
    if n < lo:
        return lo
    if n > hi:
        return hi
    return n


def format_name(first, last):
    first = first.strip().title()
    last = last.strip().title()
    return f"{first} {last}"


def format_address(street, city):
    street = street.strip().title()
    city = city.strip().title()
    return f"{street}, {city}"
