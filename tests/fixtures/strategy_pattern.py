# Strategy pattern — same pipeline, different provider implementations.
# Should be classified as strategy_candidate.

def charge_stripe(amount, currency, customer_id):
    validated = validate_amount(amount)
    request = build_payment_request(validated, currency)
    credentials = get_stripe_credentials()
    response = stripe_api.execute(request, credentials)
    parsed = parse_stripe_response(response)
    result = normalize_result(parsed)
    log_transaction(result, "stripe")
    return result


def charge_paypal(amount, currency, customer_id):
    validated = validate_amount(amount)
    request = build_payment_request(validated, currency)
    credentials = get_paypal_credentials()
    response = paypal_api.execute(request, credentials)
    parsed = parse_paypal_response(response)
    result = normalize_result(parsed)
    log_transaction(result, "paypal")
    return result


def charge_adyen(amount, currency, customer_id):
    validated = validate_amount(amount)
    request = build_payment_request(validated, currency)
    credentials = get_adyen_credentials()
    response = adyen_api.execute(request, credentials)
    parsed = parse_adyen_response(response)
    result = normalize_result(parsed)
    log_transaction(result, "adyen")
    return result
