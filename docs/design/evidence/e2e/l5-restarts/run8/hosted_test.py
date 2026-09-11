"""Transport-specific startup and delivery regression; synthetic inputs only."""
import unittest
from support import send_ready, delivered

class HostedStartup(unittest.TestCase):
    # // Regression: ca087893 applied tmux journal onboarding to hosted beta.
    def facts(self):
        activity = {'state':'idle','age':1,'session_id':'thread',
                    'runtime':{'source':'host','activity_attribution':'attributed'}}
        card = {'event':'onboarding.delivery.observed','path':'app_server',
                'stage':'submitted','card_key':{'recipient':['team','member']}}
        events = [
            {'method':'item/completed','params':{'threadId':'thread','turnId':'startup',
             'item':{'type':'userMessage','content':[{'text':'[taurhaus] recovery_card\nIdentity: beta on team; key={"recipient":["team","member"]}'}]}}},
            {'method':'turn/completed','params':{'threadId':'thread','turn':{'id':'startup','status':'completed'}}}]
        return activity, card, events

    def test_started_user_card_with_completed_turn_satisfies_startup(self):
        # // Regression: d8146e39 required item/completed; native startup may emit only item/started.
        a, card, events = self.facts()
        events[0]['method'] = 'item/started'
        self.assertTrue(send_ready([], 'beta', a, onboarding=True, transport='app_server',
                                  startup_rows=[card], host_events=events, recipient=['team','member']))
        self.assertFalse(send_ready([], 'beta', a, onboarding=True, transport='app_server',
                                   startup_rows=[card], host_events=events[:1], recipient=['team','member']))

    def test_started_message_card_is_host_exposure(self):
        # // Regression: d8146e39 also overconstrained the later-send host-card witness.
        from support import host_card_seen
        a, _, events = self.facts()
        events[0]['method'] = 'item/started'
        body = events[0]['params']['item']['content'][0]['text']
        self.assertTrue(host_card_seen(events, a['session_id'], body))
        self.assertFalse(host_card_seen(events, 'foreign-thread', body))

    def test_hosted_startup_needs_no_journal_projection(self):
        a, card, events = self.facts()
        self.assertTrue(send_ready([], 'beta', a, onboarding=True, transport='app_server',
                                  startup_rows=[card],host_events=events,recipient=['team','member']))

    def test_hosted_startup_requires_matching_card_completion_and_host_idle(self):
        a, card, events = self.facts()
        for rows, ev, activity, recipient in [([],events,a,['team','member']),
            ([card],events[:1],a,['team','member']),([card],events,a,['team','foreign']),
            ([card],events,dict(a,runtime={'source':'notify'}),['team','member']),
            ([card],events,dict(a,state='active'),['team','member'])]:
            self.assertFalse(send_ready([], 'beta',activity,onboarding=True,transport='app_server',
                                       startup_rows=rows,host_events=ev,recipient=recipient))

    def test_hosted_delivery_needs_native_receipt_and_card(self):
        a, _, _ = self.facts()
        self.assertTrue(delivered([{'stage':'native_enqueued'}],a,'thread',seat='beta',
                                  transport='app_server',card_seen=True))
        for receipts,seen in [([{'stage':'native_enqueued'}],False),
                              ([{'kind':'consumed_by_read','reader_name':'beta'}],True),([],True)]:
            self.assertFalse(delivered(receipts,a,'thread',seat='beta',transport='app_server',card_seen=seen))

    def test_later_hosted_send_accepts_native_card_without_read_receipt(self):
        a, _, events = self.facts()
        body=events[0]['params']['item']['content'][0]['text']
        accepted={'event_type':'message_accepted','payload':{'message_id':'q','body':body,'delivery_targets':[{'recipient':'beta'}]}}
        receipt={'event_type':'receipt','payload':{'message_id':'q','stage':'native_enqueued'}}
        self.assertTrue(send_ready([accepted,receipt],'beta',a,transport='app_server',host_events=events))
        self.assertFalse(send_ready([accepted],'beta',a,transport='app_server',host_events=events))
