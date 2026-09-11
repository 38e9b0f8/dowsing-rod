# False positives — these have the same SHAPE but different SEMANTICS.
# The tool should NOT classify these as high-confidence duplicates
# because the called functions are different (save vs delete, validate vs sanitize).

def save_user(user):
    """Save a user to the database."""
    data = serialize(user)
    result = db.save(data)
    log.info("User saved: %s", user.id)
    return result


def delete_user(user):
    """Delete a user from the database."""
    data = serialize(user)
    result = db.delete(data)
    log.info("User deleted: %s", user.id)
    return result


def add_x_and_y(x, y):
    return x + y


def subtract_x_and_y(x, y):
    return x - y


def validate_email(email):
    if not isinstance(email, str):
        raise TypeError("Expected string")
    if "@" not in email:
        raise ValueError("Invalid email format")
    return email.strip().lower()


def sanitize_html(html):
    if not isinstance(html, str):
        raise TypeError("Expected string")
    if "<script>" in html:
        raise ValueError("Script tags not allowed")
    return html.strip().lower()
