package instances

import (
	"encoding/json"
	"fmt"
	"io/ioutil"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/ec2"
	log "github.com/sirupsen/logrus"
)

func GetRunningInstances(client *ec2.EC2) (*ec2.DescribeInstancesOutput, error) {
	result, err := client.DescribeInstances(&ec2.DescribeInstancesInput{
		Filters: []*ec2.Filter{
			{
				Name: aws.String("instance-state-name"),
				Values: []*string{
					aws.String("running"),
				},
			},
		},
	})

	if err != nil {
		return nil, err
	}

	return result, err
}


func ListRunningInstances(profile string, region string) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Warn("Failed to initialize new session: %v", err)
		return
	}

	ec2Client := ec2.New(sess)

	runningInstances, err := GetRunningInstances(ec2Client)
	if err != nil {
		log.Warn("Couldn't retrieve running instances: %v", err)
		return
	}

	for _, reservation := range runningInstances.Reservations {
		for _, instance := range reservation.Instances {
			fmt.Println(instance)
		}
	}	
}

func GetInstanceDetails(instanceID string, profile string, region string) (*ec2.Instance){
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session: %v", err)
		return nil
	}

	ec2Client := ec2.New(sess)

	runningInstances, err := GetRunningInstances(ec2Client)
	if err != nil {
		log.Error("Couldn't retrieve running instances: %v", err)
		return nil
	}


	for _, reservation := range runningInstances.Reservations {
		for _, instance := range reservation.Instances {

			if *(instance.InstanceId) == instanceID {
				file, _ := json.MarshalIndent(instance, "", " ")
 
				_ = ioutil.WriteFile("instance.json", file, 0644)
				return instance
			}
			
		}
		
	}	
	return nil
}
