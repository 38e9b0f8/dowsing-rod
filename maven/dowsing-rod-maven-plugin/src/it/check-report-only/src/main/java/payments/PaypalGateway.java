package payments;

// Paypal payment provider implementation.
public class PaypalGateway implements PaymentGateway {
    private final PaypalClient paypalClient;
    private final AuditLog auditLog;

    public PaypalGateway(PaypalClient paypalClient, AuditLog auditLog) {
        this.paypalClient = paypalClient;
        this.auditLog = auditLog;
    }

    @Override
    public ChargeResult charge(long amount, String currency, String customerId) {
        long validated = Validation.validateAmount(amount);
        ChargeRequest request = Requests.buildChargeRequest(validated, currency, customerId);
        ProviderResponse response = paypalClient.payments().execute(request);
        if (!"succeeded".equals(response.getStatus())) {
            throw new PaymentException("Paypal charge failed: " + response.getFailureMessage());
        }
        ChargeResult result = Results.normalizeChargeResult(response);
        auditLog.record("charge", "paypal", result);
        return result;
    }

    @Override
    public RefundResult refund(String transactionId, Long amount) {
        Transaction transaction = Transactions.lookup(transactionId);
        long refundAmount = amount != null ? amount : transaction.getAmount();
        RefundRequest request = Requests.buildRefundRequest(transaction.getProviderId(), refundAmount);
        ProviderResponse response = paypalClient.sales().refund(request);
        if (!"succeeded".equals(response.getStatus())) {
            throw new PaymentException("Paypal refund failed: " + response.getFailureMessage());
        }
        RefundResult result = Results.normalizeRefundResult(response);
        auditLog.record("refund", "paypal", result);
        return result;
    }
}
