# Async function variants for duplicate detection across sync/async.

import asyncio


async def fetch_user_data(user_id):
    validated = validate_id(user_id)
    response = await http_client.get(f"/users/{validated}")
    data = parse_response(response)
    if data is None:
        raise ValueError("User not found")
    return transform(data)


async def fetch_product_data(product_id):
    validated = validate_id(product_id)
    response = await http_client.get(f"/products/{validated}")
    data = parse_response(response)
    if data is None:
        raise ValueError("Product not found")
    return transform(data)


async def fetch_order_data(order_id):
    validated = validate_id(order_id)
    response = await http_client.get(f"/orders/{validated}")
    data = parse_response(response)
    if data is None:
        raise ValueError("Order not found")
    return transform(data)
